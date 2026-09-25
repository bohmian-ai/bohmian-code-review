Imports System
Imports System.Collections.Generic

Namespace MyApp
    Public Class Customer
        Inherits BaseEntity
        Implements IComparable(Of Customer)

        Public Const DEFAULT_REGION As String = "US"
        Public Shared ReadOnly MAX_ITEMS As Integer = 100

        Private _name As String
        Public Property Name As String
            Get
                Return _name
            End Get
            Set(value As String)
                _name = value
                RaiseEvent NameChanged(Me, EventArgs.Empty)
            End Set
        End Property

        Public Event NameChanged As EventHandler

        Public Sub New(name As String)
            Me.Name = name
        End Sub

        Public Function Save() As Boolean
            Dim repo As New Repository(Of Customer)
            Return repo.Persist(Me)
        End Function

        Public Function CompareTo(other As Customer) As Integer
            Return String.Compare(Me.Name, other.Name)
        End Function
    End Class

    Public Structure Point
        Public X As Integer
        Public Y As Integer
    End Structure

    Public Enum Status
        Pending
        Active
        Closed
    End Enum

    Public Delegate Function Mapper(Of TIn, TOut)(source As TIn) As TOut
End Namespace
