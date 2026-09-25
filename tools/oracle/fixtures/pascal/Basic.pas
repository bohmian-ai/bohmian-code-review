unit BasicUnit;

interface

uses
  SysUtils, Classes;

type
  TShape = class(TObject)
  private
    FName: string;
  protected
    function GetArea: Double; virtual;
  public
    constructor Create(AName: string);
    class function Make(AName: string): TShape;
    procedure Draw; virtual; abstract;
    property Name: string read FName write FName;
  end;

  TCircle = class(TShape)
  private
    FRadius: Double;
  public
    procedure Draw; override;
    property Radius: Double read FRadius write FRadius;
  end;

  TColor = (Red, Green, Blue);

  TPoint = record
    X, Y: Integer;
  end;

  TIntArray = array of Integer;

const
  MAX_SHAPES = 100;
  DEFAULT_NAME = 'shape';

var
  GlobalCount: Integer;

implementation

constructor TShape.Create(AName: string);
begin
  FName := AName;
  GlobalCount := GlobalCount + 1;
end;

class function TShape.Make(AName: string): TShape;
begin
  Result := TShape.Create(AName);
end;

function TShape.GetArea: Double;
begin
  Result := 0.0;
end;

procedure TCircle.Draw;
begin
  WriteLn('drawing ', FName, ' radius ', FRadius);
end;

function Helper(const Value: string): string;
begin
  Result := UpperCase(Value);
end;

end.
