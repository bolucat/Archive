#!/bin/sh
# SeedFlip Glacier

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
put_template 0  "c2/d4/e5"
put_template 1  "bd/0f/0f"
put_template 2  "0f/bd/49"
put_template 3  "bd/a0/0f"
put_template 4  "0f/58/bd"
put_template 5  "bd/0f/bd"
put_template 6  "0f/bd/bd"
put_template 7  "08/0d/17"
put_template 8  "8d/ad/ce"
put_template 9  "ee/2b/2b"
put_template 10 "12/d4/52"
put_template 11 "d4/b3/12"
put_template 12 "2b/7c/ee"
put_template 13 "ee/2b/ee"
put_template 14 "12/d4/d4"
put_template 15 "0f/17/2a"

color_foreground="0f/17/2a"
color_background="f8/fa/fc"

if [ -n "$ITERM_SESSION_ID" ]; then
  # iTerm2 proprietary escape codes
  put_template_custom Pg "0f172a"
  put_template_custom Ph "f8fafc"
  put_template_custom Pi "0f172a"
  put_template_custom Pj "ffffff"
  put_template_custom Pk "0f172a"
  put_template_custom Pl "0ea5e9"
  put_template_custom Pm "0f172a"
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
