// Ünïcode comment — héllo wörld 😀
unit UnicodeUnit;

interface

uses
  SysUtils;

type
  TShape = class(TObject)
  private
    FName: string;
  public
    procedure Draw;
  end;

implementation

procedure TShape.Draw;
begin
  WriteLn('héllo wörld — ünïcode 😀');
end;

end.
