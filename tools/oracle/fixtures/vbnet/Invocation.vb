Imports System
Imports System.Linq

Public Class Invoker
    Public Function Run(items As List(Of String), target As String) As Integer
        Dim found = items.IndexOf(target)
        Dim upper = target.ToUpper()
        Dim count = items.Count(Function(x) x = target)
        Dim service As New CustomerService(1)
        Dim result = service.Fetch(found)
        Return result
    End Function
End Class
