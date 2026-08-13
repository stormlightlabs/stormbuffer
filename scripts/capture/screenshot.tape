Require sbuf

Set Shell bash
Set FontSize 16
Set Width 1000
Set Height 750
Set Padding 24
Set Margin 24
Set BorderRadius 12
Set WindowBar Colorful
Set TypingSpeed 35ms
Set CursorBlink false

Env PS1 "> "

Hide
Type "clear"
Enter
Show

Sleep 300ms
Type 'sbuf search "evolving skills" --limit 2'
Enter

Wait+Screen /Score:/
Sleep 1200ms

Screenshot screenshot.png
