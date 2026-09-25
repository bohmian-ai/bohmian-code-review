unit PartialUnit;

interface

type
  TValid = class(TObject)
  public
    procedure Good;
  end;

type
  TBroken = class(TObject)
  public
    procedure Bad( // unclosed paren breaks the second construct
end.

implementation

procedure TValid.Good;
begin
  Log('ok');
end;

end.
