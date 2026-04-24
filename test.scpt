tell application "System Events"
    set windowList to {}
    set procList to every application process whose background only is false
    set idCounter to 0
    repeat with proc in procList
        set procName to name of proc
        try
            set winList to every window of proc
            repeat with win in winList
                try
                    set winTitle to name of win
                    set winInfo to "{\"id\":" & idCounter & ",\"app\":\"" & procName & "\",\"title\":\"" & winTitle & "\"}"
                    set end of windowList to winInfo
                    set idCounter to idCounter + 1
                end try
            end repeat
        end try
    end repeat
    return "[" & my joinList(windowList, ",") & "]"
end tell

on joinList(theList, theDelimiter)
    set oldDelimiters to AppleScript's text item delimiters
    set AppleScript's text item delimiters to theDelimiter
    set theString to theList as string
    set AppleScript's text item delimiters to oldDelimiters
    return theString
end joinList
