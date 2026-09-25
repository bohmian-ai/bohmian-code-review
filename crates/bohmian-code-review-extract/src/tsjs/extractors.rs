//! The extract_* family — continuation of the Walker impl (see mod.rs for the
//! porting contract). Each function mirrors its namesake in
//! src/extraction/tree-sitter.ts; TS-file line references are from the frozen
//! port. Bug-for-bug fidelity is deliberate — fix the TS side first.

use super::{
    Extra, Scope, Variant, Walker, body_of, is_builtin_type, is_literal_receiver, is_react_hoc,
    is_variable_type, is_vue_collection_name,
};
use crate::buffers::edge_kind_index;
use crate::textutil as util;
use tree_sitter::Node;

impl<'t> Walker<'t> {
    // --- extractFunction --------------------------------------------------------

    pub(super) fn extract_function(&mut self, node: Node<'t>, name_override: Option<String>) {
        let mut name = name_override
            .clone()
            .unwrap_or_else(|| self.extract_name(node));

        // Arrow/function-expression values: resolve the name from the parent
        // variable_declarator (`export const useAuth = () => {}`).
        if name_override.is_none()
            && name == "<anonymous>"
            && matches!(node.kind(), "arrow_function" | "function_expression")
            && let Some(parent) = node.parent()
            && parent.kind() == "variable_declarator"
            && let Some(var_name) = parent.child_by_field_name("name")
        {
            name = self.text(var_name).to_string();
        }
        if name == "<anonymous>" {
            // Still walk the body: module wrappers hold named inner functions
            // and calls that would otherwise be lost (#528).
            if let Some(body) = body_of(node) {
                self.visit_function_body(body);
            }
            return;
        }

        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            signature: self.signature_of(node),
            visibility: self.visibility_of(node),
            is_exported: Some(self.is_exported(node)),
            is_async: Some(self.is_async(node)),
            is_static: self.is_static(node),
            ..Extra::default()
        };
        let Some(row) = self.create_node("function", &name, node, extra) else {
            return;
        };

        self.extract_type_annotations(node, row);
        self.extract_decorators_for(node, row);

        self.stack.push(Scope {
            row,
            kind: "function",
            name,
        });
        if let Some(body) = body_of(node) {
            self.visit_function_body(body);
        }
        self.stack.pop();
    }

    // --- reactComponentHoc / extractReactComponentNode (#841) --------------------

    /// Some(inner) when the initializer is a recognized component wrapper —
    /// inner is the inline render function, or None for `styled.x`/`memo(Ref)`.
    /// Outer None = not a component wrapper.
    fn react_component_hoc(&self, value: Node<'t>) -> Option<Option<Node<'t>>> {
        if value.kind() != "call_expression" {
            return None;
        }
        let callee = value.child_by_field_name("function")?;
        let callee_text = self.text(callee);
        if util::styled_callee().is_match(callee_text) {
            return Some(None);
        }
        if !is_react_hoc(callee_text) {
            return None;
        }
        let mut inner: Option<Node> = None;
        if let Some(args) = value.child_by_field_name("arguments") {
            for i in 0..args.named_child_count() {
                if let Some(a) = args.named_child(i)
                    && matches!(a.kind(), "arrow_function" | "function_expression")
                {
                    inner = Some(a);
                    break;
                }
            }
        }
        Some(inner)
    }

    /// Recognizes react component node syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_react_component_node(
        &mut self,
        name: &str,
        declarator: Node<'t>,
        inner_fn: Option<Node<'t>>,
        extra: Extra,
    ) {
        let Some(row) = self.create_node("component", name, declarator, extra) else {
            return;
        };
        let Some(inner) = inner_fn else { return };
        self.stack.push(Scope {
            row,
            kind: "component",
            name: name.to_string(),
        });
        if let Some(body) = body_of(inner) {
            self.visit_function_body(body);
        }
        self.stack.pop();
    }

    // --- extractClass ------------------------------------------------------------

    pub(super) fn extract_struct(&mut self, node: Node<'t>) {
        // The ArkTS struct_declaration carries name/body fields exactly like a
        // class; the generic extractStruct requires the body field.
        let Some(body) = body_of(node) else { return };
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            visibility: self.visibility_of(node),
            is_exported: Some(self.is_exported(node)),
            ..Extra::default()
        };
        let Some(row) = self.create_node("struct", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "struct",
            name,
        });
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    pub(super) fn extract_class(&mut self, node: Node<'t>) {
        let resolved_body = body_of(node); // skipBodilessClass unset for TS/JS
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            visibility: self.visibility_of(node),
            is_exported: Some(self.is_exported(node)),
            ..Extra::default()
        };
        let Some(row) = self.create_node("class", &name, node, extra) else {
            return;
        };

        self.extract_inheritance(node, row);
        self.extract_decorators_for(node, row);

        self.stack.push(Scope {
            row,
            kind: "class",
            name,
        });
        let body = resolved_body.unwrap_or(node);
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    // --- extractMethod -------------------------------------------------------------

    pub(super) fn extract_method(&mut self, node: Node<'t>) {
        if !self.inside_class_like() {
            // Object-literal methods are ephemeral: walk the body only.
            if let Some(parent) = node.parent()
                && matches!(parent.kind(), "object" | "object_expression")
            {
                if let Some(body) = body_of(node) {
                    self.visit_function_body(body);
                }
                return;
            }
            self.extract_function(node, None);
            return;
        }

        let name = self.extract_name(node);
        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            signature: self.signature_of(node),
            visibility: self.visibility_of(node),
            is_async: Some(self.is_async(node)),
            is_static: self.is_static(node),
            ..Extra::default() // methods carry no isExported (mirrors extractMethod)
        };
        let Some(row) = self.create_node("method", &name, node, extra) else {
            return;
        };

        self.extract_type_annotations(node, row);
        self.extract_decorators_for(node, row);

        self.stack.push(Scope {
            row,
            kind: "method",
            name,
        });
        if let Some(body) = body_of(node) {
            self.visit_function_body(body);
        }
        self.stack.pop();
    }

    // --- extractInterface / extractEnum / members -----------------------------------

    pub(super) fn extract_interface(&mut self, node: Node<'t>) {
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            is_exported: Some(self.is_exported(node)),
            ..Extra::default()
        };
        let Some(row) = self.create_node("interface", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "interface",
            name,
        });
        let body = body_of(node).unwrap_or(node);
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    pub(super) fn extract_enum(&mut self, node: Node<'t>) {
        let Some(body) = body_of(node) else { return };
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            visibility: self.visibility_of(node),
            is_exported: Some(self.is_exported(node)),
            ..Extra::default()
        };
        let Some(row) = self.create_node("enum", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "enum",
            name,
        });
        for i in 0..body.named_child_count() {
            let Some(child) = body.named_child(i) else {
                continue;
            };
            if matches!(child.kind(), "property_identifier" | "enum_assignment") {
                self.extract_enum_members(child);
            } else {
                self.visit_node(child);
            }
        }
        self.stack.pop();
    }

    /// Recognizes enum members syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_enum_members(&mut self, node: Node<'t>) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.text(name_node).to_string();
            self.create_node("enum_member", &name, node, Extra::default());
            return;
        }
        let mut found = false;
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i)
                && matches!(
                    child.kind(),
                    "simple_identifier" | "identifier" | "property_identifier"
                )
            {
                let name = self.text(child).to_string();
                self.create_node("enum_member", &name, child, Extra::default());
                found = true;
            }
        }
        if !found && node.named_child_count() == 0 {
            let name = self.text(node).to_string();
            self.create_node("enum_member", &name, node, Extra::default());
        }
    }

    // --- extractProperty (#808 property-classified class fields) ---------------------

    pub(super) fn extract_property(&mut self, node: Node<'t>) -> Option<(u32, String)> {
        let docstring = crate::docstring::preceding_docstring(node, self.src);
        let visibility = self.visibility_of(node);
        let is_static = Some(self.is_static(node).unwrap_or(false)); // `?? false` — always present

        let name_node = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("property"))
            .or_else(|| {
                (0..node.named_child_count())
                    .filter_map(|i| node.named_child(i))
                    .find(|c| c.kind() == "identifier")
            })?;
        let name = self.text(name_node).to_string();

        // TS/JS field definitions carry an explicit `type` field; the generic
        // scan is for other languages (#808).
        let type_text = node.child_by_field_name("type").map(|t| {
            let raw = self.text(t);
            raw.strip_prefix(':')
                .unwrap_or(raw)
                .trim_start()
                .to_string()
        });
        let signature = match &type_text {
            Some(t) => format!("{t} {name}"),
            None => name.clone(),
        };

        let row = self.create_node(
            "property",
            &name,
            node,
            Extra {
                docstring,
                signature: Some(signature),
                visibility,
                is_static,
                ..Extra::default()
            },
        )?;
        self.extract_decorators_for(node, row);
        self.extract_type_annotations(node, row);
        Some((row, name))
    }

    // --- extractVariable (TS/JS branch) ------------------------------------------------

    pub(super) fn extract_variable(&mut self, node: Node<'t>) {
        let is_const = self.is_const_decl(node);
        let kind: &'static str = if is_const { "constant" } else { "variable" };
        let docstring = crate::docstring::preceding_docstring(node, self.src);
        let is_exported = self.is_exported(node); // `?? false` — always present

        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() != "variable_declarator" {
                continue;
            }
            let Some(name_node) = child.child_by_field_name("name") else {
                continue;
            };
            let value = child.child_by_field_name("value");

            // Destructured patterns are skipped — except RTK Query generated
            // hooks (`export const { useGetXQuery } = api`).
            if matches!(name_node.kind(), "object_pattern" | "array_pattern") {
                if name_node.kind() == "object_pattern"
                    && value.map(|v| v.kind() == "identifier").unwrap_or(false)
                {
                    self.extract_rtk_hook_bindings(name_node, is_exported);
                }
                continue;
            }
            let name = self.text(name_node).to_string();

            // Arrow/function values extract as functions, named by the declarator.
            if let Some(v) = value
                && matches!(v.kind(), "arrow_function" | "function_expression")
            {
                self.extract_function(v, None);
                continue;
            }

            let init_signature = value.map(|v| util::init_signature(self.text(v)));

            // React HOC-wrapped components (#841), PascalCase-gated.
            if let Some(v) = value
                && util::pascal_case().is_match(&name)
                && let Some(inner) = self.react_component_hoc(v)
            {
                self.extract_react_component_node(
                    &name,
                    child,
                    inner,
                    Extra {
                        docstring: docstring.clone(),
                        signature: init_signature.clone(),
                        is_exported: Some(is_exported),
                        ..Extra::default()
                    },
                );
                continue;
            }

            let var_row = self.create_node(
                kind,
                &name,
                child,
                Extra {
                    docstring: docstring.clone(),
                    signature: init_signature.clone(),
                    is_exported: Some(is_exported),
                    ..Extra::default()
                },
            );
            if let Some(row) = var_row {
                self.extract_variable_type_annotation(child, row);
            }

            // Exported const object-of-functions / store shapes.
            let object_of_fns: Option<Node> = match value {
                Some(v) if matches!(v.kind(), "object" | "object_expression") => Some(v),
                Some(v) if v.kind() == "call_expression" => {
                    self.find_initializer_returned_object(v, 0)
                }
                _ => None,
            };
            let has_inline_fns = object_of_fns
                .map(|o| self.object_has_inline_functions(o))
                .unwrap_or(false);
            let extract_object_methods = is_exported && object_of_fns.is_some() && has_inline_fns;

            let rtk_endpoints = match value {
                Some(v) if v.kind() == "call_expression" => self.find_rtk_endpoints_object(v),
                _ => None,
            };
            let pinia_setup = match value {
                Some(v) if v.kind() == "call_expression" => self.find_pinia_setup_fn(v),
                _ => None,
            };
            let mut store_collections: Vec<Node> = Vec::new();
            if let Some(v) = value
                && matches!(v.kind(), "call_expression" | "new_expression")
            {
                store_collections.extend(self.find_vue_store_collection_objects(v));
            }
            if let Some(obj) = object_of_fns
                && !extract_object_methods
                && is_vue_collection_name(&name)
                && self.looks_like_vue_store_file()
            {
                store_collections.push(obj);
            }

            // Walk the initializer for calls — except the object/store shapes
            // whose members are extracted method-by-method below.
            if let Some(v) = value {
                let vk = v.kind();
                if vk != "object"
                    && vk != "object_expression"
                    && !(extract_object_methods && vk == "call_expression")
                    && rtk_endpoints.is_none()
                    && pinia_setup.is_none()
                    && store_collections.is_empty()
                {
                    self.visit_function_body(v);
                }
            }

            if extract_object_methods && let Some(obj) = object_of_fns {
                self.extract_object_literal_functions(obj);
            }
            if let Some(rtk) = rtk_endpoints {
                self.extract_rtk_endpoints(rtk);
            }
            if let Some(setup) = pinia_setup {
                self.extract_pinia_setup_body(setup);
            }
            for coll in store_collections {
                self.extract_object_literal_functions(coll);
            }
        }
    }

    /// extractRtkHookBindings — `export const { useGetXQuery } = api`.
    fn extract_rtk_hook_bindings(&mut self, pattern: Node<'t>, is_exported: bool) {
        for i in 0..pattern.named_child_count() {
            let Some(binding) = pattern.named_child(i) else {
                continue;
            };
            if binding.kind() != "shorthand_property_identifier_pattern" {
                continue;
            }
            let name = self.text(binding).to_string();
            if !util::rtk_hook_name().is_match(&name) {
                continue;
            }
            self.create_node(
                "function",
                &name,
                binding,
                Extra {
                    is_exported: Some(is_exported),
                    signature: Some("= RTK Query generated hook".to_string()),
                    ..Extra::default()
                },
            );
        }
    }

    // --- object-literal / store helpers -------------------------------------------------

    pub(super) fn extract_object_literal_functions(&mut self, obj: Node<'t>) {
        for i in 0..obj.named_child_count() {
            let Some(member) = obj.named_child(i) else {
                continue;
            };
            if member.kind() == "pair" {
                let key = member.child_by_field_name("key");
                let value = member.child_by_field_name("value");
                if let (Some(k), Some(v)) = (key, value)
                    && matches!(v.kind(), "arrow_function" | "function_expression")
                {
                    let name = util::object_key_name(self.text(k));
                    self.extract_function(v, Some(name));
                }
            } else if member.kind() == "method_definition"
                && let Some(k) = member.child_by_field_name("name")
            {
                let name = util::object_key_name(self.text(k));
                self.extract_function(member, Some(name));
            }
        }
    }

    /// Locates initializer returned object within the permitted syntax boundary and returns no match rather than crossing scopes.
    fn find_initializer_returned_object(&self, call: Node<'t>, depth: u32) -> Option<Node<'t>> {
        if depth > 4 {
            return None;
        }
        let args = call.child_by_field_name("arguments")?;
        for i in 0..args.named_child_count() {
            let Some(arg) = args.named_child(i) else {
                continue;
            };
            if matches!(arg.kind(), "arrow_function" | "function_expression") {
                if let Some(obj) = self.function_returned_object(arg) {
                    return Some(obj);
                }
            } else if arg.kind() == "call_expression"
                && let Some(obj) = self.find_initializer_returned_object(arg, depth + 1)
            {
                return Some(obj);
            }
        }
        None
    }

    /// Finds an object literal returned by a function so framework members can be synthesized.
    fn function_returned_object(&self, fn_node: Node<'t>) -> Option<Node<'t>> {
        /// Returns the object-literal node after unwrapping supported expression wrappers.
        fn as_object<'t>(n: Node<'t>) -> Option<Node<'t>> {
            match n.kind() {
                "object" | "object_expression" => Some(n),
                "parenthesized_expression" => {
                    for i in 0..n.named_child_count() {
                        if let Some(inner) = n.named_child(i).and_then(as_object) {
                            return Some(inner);
                        }
                    }
                    None
                }
                _ => None,
            }
        }
        let body = fn_node.child_by_field_name("body")?;
        if let Some(direct) = as_object(body) {
            return Some(direct);
        }
        if body.kind() == "statement_block" {
            for i in 0..body.named_child_count() {
                let Some(stmt) = body.named_child(i) else {
                    continue;
                };
                if stmt.kind() != "return_statement" {
                    continue;
                }
                for j in 0..stmt.named_child_count() {
                    if let Some(obj) = stmt.named_child(j).and_then(as_object) {
                        return Some(obj);
                    }
                }
            }
        }
        None
    }

    pub(super) fn object_has_inline_functions(&self, obj: Node) -> bool {
        for i in 0..obj.named_child_count() {
            let Some(member) = obj.named_child(i) else {
                continue;
            };
            if member.kind() == "method_definition" {
                return true;
            }
            if member.kind() == "pair"
                && let Some(v) = member.child_by_field_name("value")
                && matches!(v.kind(), "arrow_function" | "function_expression")
            {
                return true;
            }
        }
        false
    }

    /// Locates rtk endpoints object within the permitted syntax boundary and returns no match rather than crossing scopes.
    fn find_rtk_endpoints_object(&self, call: Node<'t>) -> Option<Node<'t>> {
        let callee = call.child_by_field_name("function")?;
        let callee_name = match callee.kind() {
            "identifier" => self.text(callee),
            "member_expression" => {
                let prop = callee.child_by_field_name("property").unwrap_or(callee);
                self.text(prop)
            }
            _ => "",
        };
        if callee_name != "createApi" && callee_name != "injectEndpoints" {
            return None;
        }
        let args = call.child_by_field_name("arguments")?;
        for i in 0..args.named_child_count() {
            let Some(arg) = args.named_child(i) else {
                continue;
            };
            if !matches!(arg.kind(), "object" | "object_expression") {
                continue;
            }
            for j in 0..arg.named_child_count() {
                let Some(member) = arg.named_child(j) else {
                    continue;
                };
                if member.kind() == "pair" {
                    let Some(key) = member.child_by_field_name("key") else {
                        continue;
                    };
                    if self.text(key) != "endpoints" {
                        continue;
                    }
                    if let Some(value) = member.child_by_field_name("value")
                        && matches!(value.kind(), "arrow_function" | "function_expression")
                    {
                        return self.function_returned_object(value);
                    }
                } else if member.kind() == "method_definition" {
                    let Some(key) = member.child_by_field_name("name") else {
                        continue;
                    };
                    if self.text(key) != "endpoints" {
                        continue;
                    }
                    return self.function_returned_object(member);
                }
            }
        }
        None
    }

    /// Recognizes rtk endpoints syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_rtk_endpoints(&mut self, obj: Node<'t>) {
        for i in 0..obj.named_child_count() {
            let Some(member) = obj.named_child(i) else {
                continue;
            };
            if member.kind() != "pair" {
                continue;
            }
            let key = member.child_by_field_name("key");
            let value = member.child_by_field_name("value");
            let (Some(key), Some(value)) = (key, value) else {
                continue;
            };
            if value.kind() != "call_expression" {
                continue;
            }
            let Some(callee) = value.child_by_field_name("function") else {
                continue;
            };
            if callee.kind() != "member_expression" {
                continue;
            }
            let method = self.text(callee.child_by_field_name("property").unwrap_or(callee));
            if method != "query" && method != "mutation" && method != "infiniteQuery" {
                continue;
            }
            let key_name = util::object_key_name(self.text(key));
            if let Some(handler) = self.rtk_endpoint_handler(value) {
                self.extract_function(handler, Some(key_name));
            } else {
                // Config-only endpoint: bare node spanning the builder call.
                let (sig, _) = util::slice_utf16(self.text(value), 80);
                let row = self.create_node(
                    "function",
                    &key_name,
                    value,
                    Extra {
                        signature: Some(sig),
                        ..Extra::default()
                    },
                );
                if let Some(row) = row {
                    self.stack.push(Scope {
                        row,
                        kind: "function",
                        name: key_name,
                    });
                    self.visit_function_body(value);
                    self.stack.pop();
                }
            }
        }
    }

    /// Finds the handler function nested in a Redux Toolkit endpoint definition.
    fn rtk_endpoint_handler(&self, call: Node<'t>) -> Option<Node<'t>> {
        let args = call.child_by_field_name("arguments")?;
        for i in 0..args.named_child_count() {
            let Some(arg) = args.named_child(i) else {
                continue;
            };
            if !matches!(arg.kind(), "object" | "object_expression") {
                continue;
            }
            let mut query_fn: Option<Node> = None;
            let mut query: Option<Node> = None;
            let mut first_fn: Option<Node> = None;
            for j in 0..arg.named_child_count() {
                let Some(member) = arg.named_child(j) else {
                    continue;
                };
                let mut fn_node: Option<Node> = None;
                let mut key_name = "";
                if member.kind() == "pair" {
                    if let Some(v) = member.child_by_field_name("value")
                        && matches!(v.kind(), "arrow_function" | "function_expression")
                    {
                        fn_node = Some(v);
                        if let Some(k) = member.child_by_field_name("key") {
                            key_name = self.text(k);
                        }
                    }
                } else if member.kind() == "method_definition" {
                    fn_node = Some(member);
                    if let Some(k) = member.child_by_field_name("name") {
                        key_name = self.text(k);
                    }
                }
                let Some(f) = fn_node else { continue };
                if key_name == "queryFn" {
                    query_fn = Some(f);
                } else if key_name == "query" {
                    query = Some(f);
                }
                if first_fn.is_none() {
                    first_fn = Some(f);
                }
            }
            if let Some(f) = query_fn.or(query).or(first_fn) {
                return Some(f);
            }
        }
        None
    }

    pub(super) fn looks_like_vue_store_file(&mut self) -> bool {
        if let Some(v) = self.vue_store_file {
            return v;
        }
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for m in util::vue_store_signal().find_iter(self.src) {
            seen.insert(m.as_str());
            if seen.len() >= 2 {
                break;
            }
        }
        let v = seen.len() >= 2;
        self.vue_store_file = Some(v);
        v
    }

    /// Locates vue store collection objects within the permitted syntax boundary and returns no match rather than crossing scopes.
    fn find_vue_store_collection_objects(&self, call: Node<'t>) -> Vec<Node<'t>> {
        let callee = call
            .child_by_field_name("function")
            .or_else(|| call.child_by_field_name("constructor"));
        let Some(callee) = callee else { return vec![] };
        let callee_name = match callee.kind() {
            "identifier" => self.text(callee),
            "member_expression" => {
                self.text(callee.child_by_field_name("property").unwrap_or(callee))
            }
            _ => "",
        };
        if !matches!(callee_name, "defineStore" | "createStore" | "Store") {
            return vec![];
        }
        let Some(args) = call.child_by_field_name("arguments") else {
            return vec![];
        };
        let mut objects = Vec::new();
        for i in 0..args.named_child_count() {
            let Some(arg) = args.named_child(i) else {
                continue;
            };
            if !matches!(arg.kind(), "object" | "object_expression") {
                continue;
            }
            for j in 0..arg.named_child_count() {
                let Some(member) = arg.named_child(j) else {
                    continue;
                };
                if member.kind() != "pair" {
                    continue;
                }
                let Some(key) = member.child_by_field_name("key") else {
                    continue;
                };
                if !is_vue_collection_name(self.text(key)) {
                    continue;
                }
                if let Some(value) = member.child_by_field_name("value")
                    && matches!(value.kind(), "object" | "object_expression")
                {
                    objects.push(value);
                }
            }
        }
        objects
    }

    pub(super) fn extract_store_collection_methods(&mut self, config: Node<'t>) {
        for i in 0..config.named_child_count() {
            let Some(member) = config.named_child(i) else {
                continue;
            };
            if member.kind() != "pair" {
                continue;
            }
            let Some(key) = member.child_by_field_name("key") else {
                continue;
            };
            if !is_vue_collection_name(self.text(key)) {
                continue;
            }
            if let Some(value) = member.child_by_field_name("value")
                && matches!(value.kind(), "object" | "object_expression")
            {
                self.extract_object_literal_functions(value);
            }
        }
    }

    /// Locates pinia setup fn within the permitted syntax boundary and returns no match rather than crossing scopes.
    fn find_pinia_setup_fn(&self, call: Node<'t>) -> Option<Node<'t>> {
        let callee = call.child_by_field_name("function")?;
        if callee.kind() != "identifier" || self.text(callee) != "defineStore" {
            return None;
        }
        let args = call.child_by_field_name("arguments")?;
        for i in 0..args.named_child_count() {
            let Some(arg) = args.named_child(i) else {
                continue;
            };
            if !matches!(arg.kind(), "arrow_function" | "function_expression") {
                continue;
            }
            if let Some(body) = arg.child_by_field_name("body")
                && body.kind() == "statement_block"
            {
                return Some(arg);
            }
        }
        None
    }

    /// Recognizes pinia setup body syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_pinia_setup_body(&mut self, setup: Node<'t>) {
        let Some(body) = setup.child_by_field_name("body") else {
            return;
        };
        if body.kind() != "statement_block" {
            return;
        }
        for i in 0..body.named_child_count() {
            let Some(stmt) = body.named_child(i) else {
                continue;
            };
            if stmt.kind() == "function_declaration" {
                self.extract_function(stmt, None);
            } else if is_variable_type(stmt.kind()) {
                for j in 0..stmt.named_child_count() {
                    let Some(decl) = stmt.named_child(j) else {
                        continue;
                    };
                    if decl.kind() != "variable_declarator" {
                        continue;
                    }
                    if let Some(v) = decl.child_by_field_name("value")
                        && matches!(v.kind(), "arrow_function" | "function_expression")
                    {
                        self.extract_function(v, None);
                    }
                }
            }
        }
    }

    // --- extractTypeAlias + members (#359, #634) -------------------------------------

    /// Returns skipChildren (always false on the TS path — the alias value is
    /// still traversed by the dispatcher).
    pub(super) fn extract_type_alias(&mut self, node: Node<'t>) -> bool {
        let name = self.extract_name(node);
        if name == "<anonymous>" {
            return false;
        }
        let extra = Extra {
            docstring: crate::docstring::preceding_docstring(node, self.src),
            is_exported: Some(self.is_exported(node)),
            ..Extra::default()
        };
        let Some(row) = self.create_node("type_alias", &name, node, extra) else {
            return false;
        };
        if let Some(value) = node.child_by_field_name("value") {
            self.extract_type_refs_from_subtree(value, row);
            self.extract_ts_type_alias_members(value, row, &name);
            self.extract_ts_tuple_contract_names(value, row, &name);
        }
        false
    }

    /// Recognizes ts type alias members syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_ts_type_alias_members(&mut self, value: Node<'t>, alias_row: u32, alias_name: &str) {
        let mut object_types: Vec<Node> = Vec::new();
        if value.kind() == "object_type" {
            object_types.push(value);
        } else if value.kind() == "intersection_type" {
            for i in 0..value.named_child_count() {
                if let Some(op) = value.named_child(i)
                    && op.kind() == "object_type"
                {
                    object_types.push(op);
                }
            }
        } else {
            return;
        }

        self.stack.push(Scope {
            row: alias_row,
            kind: "type_alias",
            name: alias_name.to_string(),
        });
        for obj_type in object_types {
            for i in 0..obj_type.named_child_count() {
                let Some(child) = obj_type.named_child(i) else {
                    continue;
                };
                if !matches!(child.kind(), "property_signature" | "method_signature") {
                    continue;
                }
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let member_name = self.text(name_node).to_string();
                if member_name.is_empty() {
                    continue;
                }
                let member_kind: &'static str = if child.kind() == "method_signature"
                    || self.is_ts_function_typed_property(child)
                {
                    "method"
                } else {
                    "property"
                };
                let extra = Extra {
                    docstring: crate::docstring::preceding_docstring(child, self.src),
                    signature: Some(self.text(child).to_string()),
                    qualified_name: Some(format!("{alias_name}::{member_name}")),
                    ..Extra::default()
                };
                self.create_node(member_kind, &member_name, child, extra);
                self.extract_type_annotations(child, alias_row);
            }
        }
        self.stack.pop();
    }

    /// Recognizes ts tuple contract names syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_ts_tuple_contract_names(
        &mut self,
        value: Node<'t>,
        alias_row: u32,
        alias_name: &str,
    ) {
        let mut tuples: Vec<Node> = Vec::new();
        /// Collects matching descendants in syntax order while honoring the callers traversal boundary.
        fn collect<'t>(n: Node<'t>, depth: u32, out: &mut Vec<Node<'t>>) {
            if depth > 6 {
                return;
            }
            if n.kind() == "tuple_type" {
                out.push(n);
            }
            for i in 0..n.named_child_count() {
                if let Some(c) = n.named_child(i) {
                    collect(c, depth + 1, out);
                }
            }
        }
        collect(value, 0, &mut tuples);
        if tuples.is_empty() {
            return;
        }

        self.stack.push(Scope {
            row: alias_row,
            kind: "type_alias",
            name: alias_name.to_string(),
        });
        for tuple in tuples {
            for i in 0..tuple.named_child_count() {
                let Some(entry) = tuple.named_child(i) else {
                    continue;
                };
                if entry.kind() != "generic_type" {
                    continue;
                }
                let Some(type_args) = entry.child_by_field_name("type_arguments") else {
                    continue;
                };
                for j in 0..type_args.named_child_count() {
                    let Some(arg) = type_args.named_child(j) else {
                        continue;
                    };
                    if arg.kind() != "literal_type" {
                        continue;
                    }
                    let Some(str_node) = arg.named_child(0) else {
                        continue;
                    };
                    if str_node.kind() != "string" {
                        continue;
                    }
                    let name = util::object_key_name(self.text(str_node).trim());
                    if !util::ident_dollar().is_match(&name) {
                        continue;
                    }
                    let collapsed = collapse_ws(self.text(entry));
                    let (signature, _) = util::slice_utf16(collapsed.trim(), 120);
                    let extra = Extra {
                        signature: Some(signature),
                        qualified_name: Some(format!("{alias_name}::{name}")),
                        ..Extra::default()
                    };
                    self.create_node("method", &name, entry, extra);
                }
            }
        }
        self.stack.pop();
    }

    /// Classifies whether the current syntax satisfies ts function typed property; callers use the result to select the matching upstream emission branch.
    fn is_ts_function_typed_property(&self, property_signature: Node) -> bool {
        let Some(type_anno) = property_signature.child_by_field_name("type") else {
            return false;
        };
        for i in 0..type_anno.named_child_count() {
            if let Some(inner) = type_anno.named_child(i)
                && inner.kind() == "function_type"
            {
                return true;
            }
        }
        false
    }

    // --- extractImport + binding refs ---------------------------------------------------

    pub(super) fn extract_import(&mut self, node: Node<'t>) {
        let import_text = self.text(node).trim().to_string();
        // typescriptExtractor.extractImport: the `source` field, quotes stripped
        // globally. A missing/empty module means the hook declined — no node.
        let Some(source_field) = node.child_by_field_name("source") else {
            return;
        };
        let module_name: String = self
            .text(source_field)
            .chars()
            .filter(|c| *c != '\'' && *c != '"')
            .collect();
        if module_name.is_empty() {
            return;
        }
        self.create_node(
            "import",
            &module_name,
            node,
            Extra {
                signature: Some(import_text),
                ..Extra::default()
            },
        );
        let parent = self.top_row();
        self.push_ref(
            parent,
            &module_name.clone(),
            edge_kind_index("imports").unwrap(),
            node,
        );
        self.emit_import_binding_refs(node, parent);
    }

    /// Emits one unresolved import reference per local binding, retaining aliases and source coordinates.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn emit_import_binding_refs(&mut self, node: Node<'t>, from_row: u32) {
        let clause = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "import_clause");
        let Some(clause) = clause else { return }; // side-effect import

        let imports_kind = edge_kind_index("imports").unwrap();
        let push = |w: &mut Self, name_node: Option<Node>| {
            let Some(n) = name_node else { return };
            let name = w.text(n).to_string();
            if name.is_empty() {
                return;
            }
            w.push_ref(from_row, &name, imports_kind, n);
        };

        for i in 0..clause.named_child_count() {
            let Some(child) = clause.named_child(i) else {
                continue;
            };
            match child.kind() {
                "identifier" => push(self, Some(child)),
                "named_imports" => {
                    for j in 0..child.named_child_count() {
                        let Some(spec) = child.named_child(j) else {
                            continue;
                        };
                        if spec.kind() != "import_specifier" {
                            continue;
                        }
                        let n = spec
                            .child_by_field_name("alias")
                            .or_else(|| spec.child_by_field_name("name"))
                            .or_else(|| spec.named_child(0));
                        push(self, n);
                    }
                }
                "namespace_import" => {
                    let n = (0..child.named_child_count())
                        .filter_map(|k| child.named_child(k))
                        .find(|c| c.kind() == "identifier")
                        .or_else(|| child.named_child(0));
                    push(self, n);
                }
                _ => {}
            }
        }
    }

    pub(super) fn emit_re_export_refs(&mut self, node: Node<'t>) {
        let from_row = self.top_row();
        let clause = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "export_clause");
        let Some(clause) = clause else { return }; // `export * from './y'`
        let imports_kind = edge_kind_index("imports").unwrap();
        for i in 0..clause.named_child_count() {
            let Some(spec) = clause.named_child(i) else {
                continue;
            };
            if spec.kind() != "export_specifier" {
                continue;
            }
            let name_node = spec
                .child_by_field_name("name")
                .or_else(|| spec.named_child(0));
            let Some(n) = name_node else { continue };
            let name = self.text(n).to_string();
            if name.is_empty() || name == "default" {
                continue;
            }
            self.push_ref(from_row, &name, imports_kind, n);
        }
    }

    /// The ArkTS build()-DSL slice of extractCall (tree-sitter.ts arkts
    /// branch): arkui_component_expression components + chained attributes,
    /// fluent chains on call results, and detached leading-dot chains.
    /// Returns true when the node was fully handled.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn extract_arkts_call(&mut self, node: Node<'t>) -> bool {
        let caller = self.top_row();
        let calls_kind = edge_kind_index("calls").unwrap();
        let is_on_attr = |name: &str| {
            name.starts_with("on")
                && name
                    .chars()
                    .nth(2)
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
        };
        let emit_attr = |w: &mut Self, name_node: Node<'t>| {
            let attr_name = w.text(name_node);
            if attr_name.is_empty() {
                return;
            }
            w.push_ref(caller, &format!(".{attr_name}"), calls_kind, name_node);
        };
        let emit_this_handlers = |w: &mut Self, args: Option<Node<'t>>| {
            let Some(args) = args else { return };
            for j in 0..args.named_child_count() {
                let Some(arg) = args.named_child(j) else {
                    continue;
                };
                if arg.kind() != "member_expression" {
                    continue;
                }
                let obj = arg.child_by_field_name("object");
                let prop = arg.child_by_field_name("property");
                if let (Some(obj), Some(prop)) = (obj, prop)
                    && obj.kind() == "this"
                {
                    w.push_ref(caller, w.text(prop), calls_kind, arg);
                }
            }
        };

        // Shape 1: arkui_component_expression with property/arguments pairs.
        if node.kind() == "arkui_component_expression" {
            let component_field = node.child_by_field_name("function");
            if let Some(component_field) = component_field
                && component_field.kind() == "identifier"
            {
                self.push_ref(caller, self.text(component_field), calls_kind, node);
            }
            for i in 0..node.child_count() {
                let Some(child) = node.child(i) else { continue };
                if child.kind() != "property_identifier" {
                    continue;
                }
                emit_attr(self, child);
                let name = self.text(child).to_string();
                if is_on_attr(&name) {
                    let mut args: Option<Node<'t>> = None;
                    for k in i + 1..node.child_count() {
                        let Some(next) = node.child(k) else {
                            continue;
                        };
                        if next.kind() == "property_identifier" {
                            break;
                        }
                        if next.kind() == "arguments" {
                            args = Some(next);
                            break;
                        }
                    }
                    emit_this_handlers(self, args);
                }
            }
            return true;
        }

        // Shape 2: fluent chain on a call result.
        if node.kind() == "call_expression" {
            let func = node.child_by_field_name("function");
            if let Some(func) = func
                && matches!(
                    func.kind(),
                    "member_expression" | "arkui_dsl_decorator_member_expression"
                )
            {
                let obj = func.child_by_field_name("object");
                let prop = func.child_by_field_name("property");
                if let Some(prop) = prop
                    && (func.kind() == "arkui_dsl_decorator_member_expression"
                        || matches!(
                            obj.map(|o| o.kind()),
                            Some("call_expression") | Some("arkui_component_expression")
                        ))
                {
                    emit_attr(self, prop);
                    let name = self.text(prop).to_string();
                    if is_on_attr(&name) {
                        emit_this_handlers(self, node.child_by_field_name("arguments"));
                    }
                    return true;
                }
            }
            // The INNERMOST call of a proper-form detached chain has a BARE
            // identifier function.
            if let Some(func) = func
                && func.kind() == "identifier"
            {
                let mut p = node.parent();
                while let Some(parent) = p
                    && matches!(parent.kind(), "member_expression" | "call_expression")
                {
                    p = parent.parent();
                }
                if p.map(|x| x.kind()) == Some("leading_dot_expression") {
                    emit_attr(self, func);
                    let name = self.text(func).to_string();
                    if is_on_attr(&name) {
                        emit_this_handlers(self, node.child_by_field_name("arguments"));
                    }
                    return true;
                }
            }
            // Not a chained attribute — fall through to the generic call paths.
        }

        // Shape 3: detached chain segment — leading_dot_expression.
        if node.kind() == "leading_dot_expression" {
            let only = if node.named_child_count() == 1 {
                node.named_child(0)
            } else {
                None
            };
            if let Some(only) = only
                && only.kind() == "identifier"
            {
                emit_attr(self, only);
                let name = self.text(only).to_string();
                if is_on_attr(&name) {
                    let stmt = node.parent(); // expression_statement
                    let next_stmt = stmt.and_then(|s| s.next_named_sibling());
                    let paren = next_stmt.and_then(|s| s.named_child(0));
                    if paren.map(|p| p.kind()) == Some("parenthesized_expression") {
                        emit_this_handlers(self, paren);
                    }
                }
            }
            return true;
        }

        false
    }

    pub(super) fn extract_call(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }

        // ArkTS build()-DSL handling (tree-sitter.ts arkts branch): chained
        // UI-attribute names are emitted dot-prefixed so they can ONLY resolve
        // to decorator-marked attribute helpers.
        if self.variant == Variant::Arkts && self.extract_arkts_call(node) {
            return;
        }

        let func = node
            .child_by_field_name("function")
            .or_else(|| node.named_child(0));
        let mut callee_name = String::new();

        if let Some(func) = func {
            if func.kind() == "member_expression" {
                let property = func
                    .child_by_field_name("property")
                    .or_else(|| func.child_by_field_name("field"))
                    .or_else(|| func.named_child(1));
                if let Some(property) = property {
                    let method_name = self.text(property);
                    let receiver = func
                        .child_by_field_name("object")
                        .or_else(|| func.child_by_field_name("operand"))
                        .or_else(|| func.child_by_field_name("argument"))
                        .or_else(|| func.named_child(0));
                    // Literal receivers call builtins, never project symbols (#1230).
                    if let Some(r) = receiver
                        && is_literal_receiver(r.kind())
                    {
                        return;
                    }
                    let recv_ident = receiver.filter(|r| {
                        matches!(
                            r.kind(),
                            "identifier" | "simple_identifier" | "field_identifier"
                        )
                    });
                    if let Some(r) = recv_ident {
                        let receiver_name = self.text(r);
                        if !matches!(receiver_name, "self" | "this" | "cls" | "super") {
                            callee_name = format!("{receiver_name}.{method_name}");
                        } else {
                            callee_name = method_name.to_string();
                        }
                    } else {
                        // (the call-receiver re-encode branches are other
                        // languages'; TS/JS keeps the bare method name)
                        callee_name = method_name.to_string();
                    }
                }
            } else {
                callee_name = self.text(func).to_string();
            }
        }

        // Parenthesized-callee normalization (`(fn)()` → fn).
        if !callee_name.is_empty()
            && let Some(c) = util::paren_conversion().captures(&callee_name)
        {
            callee_name = c[1].to_string();
        }

        if !callee_name.is_empty() {
            self.push_call_ref(&callee_name.clone(), node);
        }
    }

    // --- extractInstantiation -----------------------------------------------------------

    pub(super) fn extract_instantiation(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }
        let ctor = node
            .child_by_field_name("constructor")
            .or_else(|| node.child_by_field_name("type"))
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.named_child(0));
        let Some(ctor) = ctor else { return };

        let mut class_name = self.text(ctor).to_string();
        // `new Map<K, V>()` → Map.
        if let Some(lt) = class_name.find('<')
            && lt > 0
        {
            class_name.truncate(lt);
        }
        // `new ns.Foo()` → Foo.
        let last_dot = class_name
            .rfind('.')
            .map(|i| i as isize)
            .unwrap_or(-1)
            .max(class_name.rfind("::").map(|i| i as isize).unwrap_or(-1));
        if last_dot >= 0 {
            class_name = class_name[(last_dot as usize + 1)..].to_string();
            // TS: .replace(/^[:.]/, '') — one leading colon-or-dot.
            if class_name.starts_with(':') || class_name.starts_with('.') {
                class_name.remove(0);
            }
        }
        let class_name = class_name.trim().to_string();
        if !class_name.is_empty() {
            let from = self.top_row();
            self.push_ref(
                from,
                &class_name,
                edge_kind_index("instantiates").unwrap(),
                node,
            );
        }
    }

    // --- extractDecoratorsFor --------------------------------------------------------------

    pub(super) fn extract_decorators_for(&mut self, decl: Node<'t>, decorated_row: u32) {
        // 1. Direct children (method/property style).
        for i in 0..decl.named_child_count() {
            let Some(child) = decl.named_child(i) else {
                continue;
            };
            self.consider_decorator(child, decorated_row);
            if child.kind() == "modifiers" {
                for j in 0..child.named_child_count() {
                    if let Some(m) = child.named_child(j) {
                        self.consider_decorator(m, decorated_row);
                    }
                }
            }
        }
        // 2. Preceding siblings (TypeScript class style), stopping at the
        //    first non-decorator so an earlier declaration's decorators never
        //    leak in. Matching by startIndex, not object identity.
        let Some(parent) = decl.parent() else { return };
        let decl_start = decl.start_byte();
        let mut decl_idx: isize = -1;
        for i in 0..parent.named_child_count() {
            if let Some(sib) = parent.named_child(i)
                && sib.start_byte() == decl_start
            {
                decl_idx = i as isize;
                break;
            }
        }
        if decl_idx > 0 {
            let mut j = decl_idx - 1;
            while j >= 0 {
                let Some(sib) = parent.named_child(j as usize) else {
                    j -= 1;
                    continue;
                };
                if !matches!(sib.kind(), "decorator" | "annotation" | "marker_annotation") {
                    break;
                }
                self.consider_decorator(sib, decorated_row);
                j -= 1;
            }
        }
    }

    /// Emits decorator references only for decorator syntax attached to the current declaration.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn consider_decorator(&mut self, n: Node<'t>, decorated_row: u32) {
        if !matches!(
            n.kind(),
            "decorator" | "annotation" | "marker_annotation" | "attribute"
        ) {
            return;
        }
        let mut target: Option<Node> = None;
        for i in 0..n.named_child_count() {
            let Some(child) = n.named_child(i) else {
                continue;
            };
            if child.kind() == "call_expression" {
                target = child
                    .child_by_field_name("function")
                    .or_else(|| child.named_child(0));
                if target.is_some() {
                    break;
                }
            }
            if matches!(
                child.kind(),
                "identifier"
                    | "member_expression"
                    | "scoped_identifier"
                    | "navigation_expression"
                    | "user_type"
                    | "type_identifier"
            ) {
                target = Some(child);
                break;
            }
        }
        let Some(target) = target else { return };
        let mut name = self.text(target).to_string();
        if let Some(lt) = name.find('<')
            && lt > 0
        {
            name.truncate(lt);
        }
        let last_dot = name
            .rfind('.')
            .map(|i| i as isize)
            .unwrap_or(-1)
            .max(name.rfind("::").map(|i| i as isize).unwrap_or(-1));
        if last_dot >= 0 {
            name = name[(last_dot as usize + 1)..].to_string();
            if name.starts_with(':') || name.starts_with('.') {
                name.remove(0);
            }
        }
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.push_ref(
            decorated_row,
            &name,
            edge_kind_index("decorates").unwrap(),
            n,
        );
    }

    // --- extractInheritance (TS/JS clauses) ---------------------------------------------------

    pub(super) fn extract_inheritance(&mut self, node: Node<'t>, class_row: u32) {
        let extends_kind = edge_kind_index("extends").unwrap();
        let implements_kind = edge_kind_index("implements").unwrap();
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            match child.kind() {
                // TS `extends_clause` (the other spellings are other grammars').
                "extends_clause" | "superclass" | "base_clause" | "extends_interfaces" => {
                    if let Some(target) = child.named_child(0) {
                        let name = self.text(target).to_string();
                        self.push_ref(class_row, &name, extends_kind, target);
                    }
                }
                "implements_clause"
                | "class_interface_clause"
                | "super_interfaces"
                | "interfaces" => {
                    for j in 0..child.named_child_count() {
                        if let Some(iface) = child.named_child(j) {
                            let name = self.text(iface).to_string();
                            self.push_ref(class_row, &name, implements_kind, iface);
                        }
                    }
                }
                // JS `class Foo extends Bar` — class_heritage holds a bare
                // identifier without an extends_clause wrapper.
                "identifier" | "type_identifier" if node.kind() == "class_heritage" => {
                    let name = self.text(child).to_string();
                    self.push_ref(class_row, &name, extends_kind, child);
                }
                // TS class_heritage wraps extends/implements — recurse.
                "field_declaration_list" | "class_heritage" => {
                    self.extract_inheritance(child, class_row);
                }
                _ => {}
            }
        }
    }

    // --- type annotations (#381 — TS family only) ----------------------------------------------

    pub(super) fn extract_type_annotations(&mut self, node: Node<'t>, from_row: u32) {
        if !self.variant.is_ts() {
            return;
        }
        if let Some(params) = node.child_by_field_name("parameters") {
            self.extract_type_refs_from_subtree(params, from_row);
        }
        if let Some(ret) = node.child_by_field_name("return_type") {
            self.extract_type_refs_from_subtree(ret, from_row);
        }
        let type_annotation = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "type_annotation");
        if let Some(ta) = type_annotation {
            self.extract_type_refs_from_subtree(ta, from_row);
        }
    }

    pub(super) fn extract_variable_type_annotation(&mut self, node: Node<'t>, from_row: u32) {
        if !self.variant.is_ts() {
            return;
        }
        let type_annotation = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "type_annotation");
        if let Some(ta) = type_annotation {
            self.extract_type_refs_from_subtree(ta, from_row);
        }
    }

    /// Recognizes type refs from subtree syntax and emits its symbol, containment, and reference facts in source order.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn extract_type_refs_from_subtree(&mut self, node: Node<'t>, from_row: u32) {
        if node.kind() == "type_identifier" {
            let type_name = self.text(node).to_string();
            if !type_name.is_empty() && !is_builtin_type(&type_name) {
                self.push_ref(
                    from_row,
                    &type_name,
                    edge_kind_index("references").unwrap(),
                    node,
                );
            }
            return;
        }
        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i) {
                self.extract_type_refs_from_subtree(c, from_row);
            }
        }
    }
}

/// `.replace(/\s+/g, ' ')` for the tuple-contract signature.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

/// DECORATED_MEMBER_TYPES (languages/arkts.ts) — declarations whose
/// decorators are surfaced on the node's `decorators` list.
fn is_decorated_member_type(kind: &str) -> bool {
    matches!(
        kind,
        "struct_declaration"
            | "public_field_definition"
            | "method_definition"
            | "function_declaration"
    )
}

impl<'t> Walker<'t> {
    /// collectDecoratorNames (languages/arkts.ts): the decorator names for a
    /// declaration, from direct `decorator` children AND preceding `decorator`
    /// siblings (the backwards walk stops at the first non-decorator so an
    /// earlier declaration's decorators never leak in). Only the
    /// DECORATED_MEMBER_TYPES declarations participate.
    pub(super) fn collect_decorator_names(&self, node: Node<'t>) -> Option<Vec<String>> {
        if !is_decorated_member_type(node.kind()) {
            return None;
        }
        let mut names: Vec<String> = Vec::new();
        let name_of = |w: &Self, dec: Node<'t>| -> Option<String> {
            for i in 0..dec.named_child_count() {
                let Some(child) = dec.named_child(i) else {
                    continue;
                };
                if child.kind() == "identifier" {
                    return Some(w.text(child).to_string());
                }
                if child.kind() == "call_expression" {
                    // `@StorageLink('theme')` / `@Extend(Text)` — the
                    // decorator name is the callee.
                    if let Some(func) = child.child_by_field_name("function")
                        && func.kind() == "identifier"
                    {
                        return Some(w.text(func).to_string());
                    }
                }
            }
            None
        };

        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "decorator"
                && let Some(n) = name_of(self, child)
            {
                names.push(n);
            }
        }

        // Preceding decorator siblings (identity by start offset, since
        // wrappers are re-created per access).
        if let Some(parent) = node.parent() {
            let start = node.start_byte();
            let mut idx = -1i64;
            for i in 0..parent.named_child_count() {
                if let Some(sib) = parent.named_child(i)
                    && sib.start_byte() == start
                {
                    idx = i as i64;
                    break;
                }
            }
            for i in (0..idx).rev() {
                let Some(sib) = parent.named_child(i as usize) else {
                    break;
                };
                if sib.kind() != "decorator" {
                    break;
                }
                if let Some(n) = name_of(self, sib) {
                    names.insert(0, n);
                }
            }
        }

        if names.is_empty() { None } else { Some(names) }
    }
}
