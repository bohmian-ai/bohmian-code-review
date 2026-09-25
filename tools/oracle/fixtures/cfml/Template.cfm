<cfif isDefined("url.action")>
  <cfset message = "hi">
</cfif>
<cfoutput>#message#</cfoutput>
<cffunction name="helper">
  <cfreturn "x">
</cffunction>
<cfscript>
  function topLevel() {
    return helper();
  }
</cfscript>
