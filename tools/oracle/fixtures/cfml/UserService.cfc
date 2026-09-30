<cfcomponent extends="BaseService" implements="IUserService">
  <cffunction name="save" access="public" returntype="User">
    <cfargument name="user" type="User" required="true">
    <cfscript>
      var svc = new UserDao();
      return svc.save(user);
    </cfscript>
  </cffunction>
  <cffunction name="list" returntype="array">
    <cfquery name="q" datasource="ds">
      SELECT id FROM users WHERE active = 1
    </cfquery>
    <cfreturn q>
  </cffunction>
</cfcomponent>
