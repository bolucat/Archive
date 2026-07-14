#!/bin/sh
# Pierre Dark

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
put_template 0  "14/14/15"
put_template 1  "ff/2e/3f"
put_template 2  "0d/be/4e"
put_template 3  "ff/ca/00"
put_template 4  "00/9f/ff"
put_template 5  "c6/35/e4"
put_template 6  "08/c0/ef"
put_template 7  "c6/c6/c8"
put_template 8  "6c/6c/71"
put_template 9  "ff/67/62"
put_template 10 "5e/cc/71"
put_template 11 "ff/d4/52"
put_template 12 "69/b1/ff"
put_template 13 "d5/68/ea"
put_template 14 "68/cd/f2"
put_template 15 "fb/fb/fb"

color_foreground="fb/fb/fb"
color_background="07/07/07"

if [ -n "$ITERM_SESSION_ID" ]; then
  # iTerm2 proprietary escape codes
  put_template_custom Pg "fbfbfb"
  put_template_custom Ph "070707"
  put_template_custom Pi "fbfbfb"
  put_template_custom Pj "19283c"
  put_template_custom Pk "fbfbfb"
  put_template_custom Pl "009fff"
  put_template_custom Pm "070707"
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
