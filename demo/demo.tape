# VHS tape file for generating trev demo GIF
# https://github.com/charmbracelet/vhs
#
# Usage:
#   mise run demo

Output demo/demo.gif
Output demo/demo.mp4

Set Shell "bash"
Set Theme "iceberg-dark"
Set FontFamily "Moralerspace Neon"
Set FontSize 14
Set Width 1200
Set Height 800
Set Padding 10
Set Framerate 60
Set CursorBlink false
Set TypingSpeed 80ms

# Launch trev
Type "trev --config demo/config.yml --no-restore /tmp/trev-demo"
Enter
Sleep 1.5s

# =============================================
# 1. internal/ → handler/ → health files
# =============================================

# Move to internal/ and expand
Type "j"
Sleep 400ms
Type "l"
Sleep 800ms

# Move to handler/ and expand
Type "j"
Sleep 400ms
Type "l"
Sleep 800ms

# Browse health.go → health_test.go
Type "j"
Sleep 500ms
Sleep 800ms
Type "j"
Sleep 500ms
Sleep 800ms

# =============================================
# 2. Navigate to web/
# =============================================
Type "jjj"
Sleep 500ms

# =============================================
# 3. web/ — expand step by step
# =============================================
Type "E"
Sleep 1s

# Move to src/ and expand deeper
Type "j"
Sleep 400ms
Type "E"
Sleep 1s

# Move to components/ and expand
Type "j"
Sleep 400ms
Type "l"
Sleep 800ms

# =============================================
# 4. Toggle ignored files via help menu
# =============================================
Type "?"
Sleep 1s
Type "/"
Sleep 400ms
Type "ignored"
Sleep 600ms
Enter
Sleep 400ms
Enter
Sleep 1s

# =============================================
# 5. Navigate to Button.tsx for preview
# =============================================
Type "j"
Sleep 500ms
Sleep 1.2s

# Hero screenshot
Screenshot demo/screenshot.png

# Toggle ignored back via help menu
Type "?"
Sleep 1s
Type "/"
Sleep 400ms
Type "ignored"
Sleep 600ms
Enter
Sleep 400ms
Enter
Sleep 600ms

# =============================================
# 6. Browse: scroll preview, move to test file
# =============================================
Type "JJJ"
Sleep 500ms
Type "K"
Sleep 500ms

# Move to Button.test.tsx (dimmed)
Type "j"
Sleep 1s

# =============================================
# 7. Fuzzy search
# =============================================
Type "/"
Sleep 500ms
Type "useAuth"
Sleep 1s
Enter
Sleep 1.2s

# =============================================
# 8. File operations — create, copy, bulk delete
# =============================================

# Clear search filter
Escape
Sleep 600ms

# Go up to hooks/
Type "k"
Sleep 500ms

# Create a new file
Type "a"
Sleep 600ms
Type "useTheme.ts"
Sleep 800ms
Enter
Sleep 1s

# Move to useTheme.ts
Type "jj"
Sleep 500ms
Sleep 800ms

# Yank (copy) it
Type "y"
Sleep 800ms

# Navigate to utils/ and paste
Type "j"
Sleep 500ms
Type "p"
Sleep 1s

# Mark useTheme.ts in hooks/ (go up 1)
Type "k"
Sleep 500ms
Type " "
Sleep 600ms

# Mark useTheme.ts in utils/ (Space auto-advanced, so 3 x j)
Type "jjj"
Sleep 500ms
Type " "
Sleep 600ms

# Delete both marked files
Type "d"
Sleep 1s
Type "y"
Sleep 1s

# Undo the delete
Type "u"
Sleep 1.2s

Sleep 1s

# Quit
Type "q"
Sleep 300ms
