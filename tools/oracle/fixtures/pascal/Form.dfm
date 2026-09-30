object MainForm: TMainForm
  Left = 0
  Top = 0
  Caption = 'Main'
  object Panel1: TPanel
    Align = alTop
    object Button1: TButton
      Caption = 'Go'
      OnClick = Button1Click
    end
    object Button2: TButton
      Caption = 'Stop'
      OnClick = Button2Click
    end
  end
  object StatusBar1: TStatusBar
    SimplePanel = True
  end
end
