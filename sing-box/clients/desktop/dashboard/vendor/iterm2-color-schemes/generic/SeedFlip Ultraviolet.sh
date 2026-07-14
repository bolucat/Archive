#!/bin/sh
# SeedFlip Ultraviolet

# source for these helper functions:
# https://github.com/chriskempson/base16-shell/blob/master/templates/default.mustache
if [ -n "$TMUX" ]; then
  # Tell tmux to pass the escape sequences through
  # (Source: http://permalink.gmane.org/gmane.comp.terminal-emulators.tmux.user/1324)
  put_template() { printf '\033Ptmux;\033\033]4;%d;rgb:%s\033\033\\\033\\' $@; }
  put_template_var() { printf '\033Ptmux;\033\033]%d;rgb:%s\033\033\\\033\\' $@; }
  put_template_custom() { printf '\033Ptmux;\033\033]%s%s\033\033\\\033\\' $@; }
elif [ "${TERM%%[-.]*}" = "screen" ]; then
  # GNU screen (screen, screen-256color, screen-256color-bce)
  put_template() { printf '\033P\033]4;%d;rgb:%s\007\033\\' $@; }
  put_template_var() { printf '\033P\033]%d;rgb:%s\007\033\\' $@; }
  put_template_custom() { printf '\033P\033]%s%s\007\033\\' $@; }
elif [ "${TERM%%-*}" = "linux" ]; then
  put_template() { [ $1 -lt 16 ] && printf "\e]P%x%s" $1 $(echo $2 | sed 's/\///g'); }
  put_template_var() { true; }
  put_template_custom() { true; }
else
  put_template() { printf '\033]4;%d;rgb:%s\033\\' $@; }
  put_template_var() { printf '\033]%d;rgb:%s\033\\' $@; }
  put_template_custom() { printf '\033]%s%s\033\\' $@; }
fi

# 16 color space
put_template 0  "05/00/0a"
put_template 1  "f0/42/42"
put_template 2  "42/f0/7c"
put_template 3  "f0/d3/42"
put_template 4  "42/8b/f0"
put_template 5  "f0/42/f0"
put_template 6  "42/f0/f0"
put_template 7  "f6/f3/fb"
put_template 8  "5b/26/87"
put_template 9  "f5/89/89"
put_template 10 "89/f5/ad"
put_template 11 "f5/e3/89"
put_template 12 "89/b6/f5"
put_template 13 "f5/89/f5"
put_template 14 "89/f5/f5"
put_template 15 "ff/ff/ff"

color_foreground="e8/e0/f5"
color_background="0b/00/14"

if [ -n "$ITERM_SESSION_ID" ]; then
  # iTerm2 proprietary escape codes
  put_template_custom Pg "e8e0f5"
  put_template_custom Ph "0b0014"
  put_template_custom Pi "e8e0f5"
  put_template_custom Pj "13082a"
  put_template_custom Pk "e8e0f5"
  put_template_custom Pl "c77dff"
  put_template_custom Pm "0b0014"
else
  put_template_var 10 $color_foreground
  put_template_var 11 $color_background
  if [ "${TERM%%-*}" = "rxvt" ]; then
    put_template_var 708 $color_background # internal border (rxvt)
  fi
  put_template_custom 12 ";7" # cursor (reverse video)
fi

# clean up
unset -f put_template
unset -f put_template_var
unset -f put_template_custom

unset color_foreground
unset color_background
