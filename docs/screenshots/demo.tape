# Records docs/screenshots/demo.gif. From the repository root:
#   cargo build --release && vhs docs/screenshots/demo.tape
Output "docs/screenshots/demo.gif"
Set Shell "bash"
Set FontSize 18
Set Width 1200
Set Height 660
Set Padding 28
Set Theme "Dracula"
Set TypingSpeed 40ms
Hide
Type "source docs/screenshots/demo-setup.sh && clear"
Enter
Show
Sleep 600ms
Type "git diff --no-ext-diff deploy.yaml   # plain git"
Sleep 500ms
Enter
Sleep 4.5s
Type "clear"
Enter
Sleep 300ms
Type "git diff deploy.yaml   # with datadiff"
Sleep 500ms
Enter
Sleep 6s
