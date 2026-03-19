#!/bin/sh

script_action=${1}

# 彩色日志函数
print_info()  { echo -e "\e[1;32m[INFO]\e[0m $1"; }
print_warn()  { echo -e "\e[1;33m[WARN]\e[0m $1"; }
print_error() { echo -e "\e[1;31m[ERROR]\e[0m $1"; }

# curl 选项：根据 insecure_skip_verify 决定是否加 -k
CURL_OPTS="--ipv4 -fSL --connect-timeout 10"
insecure=$(uci -q get mosdns.config.insecure_skip_verify)
[ "$insecure" = "1" ] && CURL_OPTS="$CURL_OPTS -k"

logfile_path() (
    configfile=$(uci -q get mosdns.config.configfile)
    if [ "$configfile" = "/var/etc/mosdns.json" ]; then
        uci -q get mosdns.config.log_file
    else
        [ ! -f /etc/mosdns/config_custom.yaml ] && exit 1
        awk '/^log:/{f=1;next}f==1{if($0~/file:/){print;exit}if($0~/^[^ ]/)exit}' /etc/mosdns/config_custom.yaml | grep -Eo "/[^'\"]+"
    fi
)

interface_dns() {
    if [ "$(uci -q get mosdns.config.custom_local_dns)" = 1 ]; then
        uci -q get mosdns.config.local_dns
    else
        local dns
        peerdns=$(uci -q get network.wan.peerdns)
        proto=$(uci -q get network.wan.proto)
        if [ "$peerdns" = 0 ] || [ "$proto" = "static" ]; then
            dns=$(uci -q get network.wan.dns 2>/dev/null)
        else
            interface_status=$(ubus call network.interface.wan status)
            dns=$(echo "$interface_status" | jsonfilter -e "@['dns-server'][0]" 2>/dev/null)
            local dns2
            dns2=$(echo "$interface_status" | jsonfilter -e "@['dns-server'][1]" 2>/dev/null)
            [ -n "$dns2" ] && dns="$dns $dns2"
        fi
        if [ -z "$dns" ]; then
            echo "119.29.29.29 223.5.5.5"
        else
            echo "$dns"
        fi
    fi
}

get_adlist() (
    adblock=$(uci -q get mosdns.config.adblock)
    if [ "$adblock" = 1 ]; then
        mkdir -p /etc/mosdns/rule/adlist
        ad_source=$(uci -q get mosdns.config.ad_source)
        for url in $ad_source;
        do
            if [ $(echo $url) = 'geosite.dat' ]; then
                echo "/var/mosdns/geosite_category-ads-all.txt"
            elif echo "$url" | grep -Eq "^file://" ; then
                echo "$url" | sed 's/file:\/\///'
            else
                echo "/etc/mosdns/rule/adlist/$(basename $url)"
                [ ! -f "/etc/mosdns/rule/adlist/$(basename $url)" ] && touch /etc/mosdns/rule/adlist/$(basename $url)
            fi
        done
    else
        rm -rf /etc/mosdns/rule/adlist /etc/mosdns/rule/.ad_source
        touch /var/mosdns/disable-ads.txt
        echo "/var/mosdns/disable-ads.txt"
    fi
)

adlist_update() {
    [ "$(uci -q get mosdns.config.adblock)" != 1 ] && return 0
    local lock_file=/var/lock/mosdns_ad_update.lock
    ad_source=$(uci -q get mosdns.config.ad_source)
    : > /etc/mosdns/rule/.ad_source
    if [ -f "$lock_file" ]; then
        local pid
        pid=$(cat "$lock_file" 2>/dev/null)
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            has_update=0
            return 0
        fi
        rm -f "$lock_file"
    fi
    echo $$ > "$lock_file"
    AD_TMPDIR=$(mktemp -d) || exit 1
    has_update=0
    for url in $ad_source;
    do
        if [ "$url" != "geosite.dat" ] && [ $(echo "$url" | grep -c -E "^file://") -eq 0 ]; then
            has_update=1
            echo "$url" >> /etc/mosdns/rule/.ad_source
            filename=$(basename $url)
            if echo "$url" | grep -Eq "^https://raw.githubusercontent.com" ; then
                [ -n "$(uci -q get mosdns.config.github_proxy)" ] && mirror="$(uci -q get mosdns.config.github_proxy)/"
            else
                mirror=""
            fi
            print_info "Downloading $mirror$url"
            curl --connect-timeout 5 -m 90 $CURL_OPTS -o "$AD_TMPDIR/$filename" "$mirror$url"
        fi
    done
    if [ $? -ne 0 ]; then
        print_error "Rules download failed."
        rm -rf "$AD_TMPDIR" "$lock_file"
        exit 1
    else
        [ $has_update -eq 1 ] && {
            mkdir -p /etc/mosdns/rule/adlist
            rm -rf /etc/mosdns/rule/adlist/*
            \cp $AD_TMPDIR/* /etc/mosdns/rule/adlist
        }
    fi
    rm -rf "$AD_TMPDIR" "$lock_file"
}

geodat_update() (
    TMPDIR=$(mktemp -d) || exit 1
    [ -n "$(uci -q get mosdns.config.github_proxy)" ] && mirror="$(uci -q get mosdns.config.github_proxy)/"
    # geoip.dat - cn-private
    print_info "Downloading ${mirror}https://github.com/Loyalsoldier/geoip/releases/latest/download/geoip-only-cn-private.dat"
    curl --connect-timeout 5 -m 60 $CURL_OPTS -o "$TMPDIR/geoip.dat" "${mirror}https://github.com/Loyalsoldier/geoip/releases/latest/download/geoip-only-cn-private.dat"
    [ $? -ne 0 ] && rm -rf "$TMPDIR" && exit 1
    # checksum - geoip.dat
    print_info "Downloading ${mirror}https://github.com/Loyalsoldier/geoip/releases/latest/download/geoip-only-cn-private.dat.sha256sum"
    curl --connect-timeout 5 -m 10 $CURL_OPTS -o "$TMPDIR/geoip.dat.sha256sum" "${mirror}https://github.com/Loyalsoldier/geoip/releases/latest/download/geoip-only-cn-private.dat.sha256sum"
    [ $? -ne 0 ] && rm -rf "$TMPDIR" && exit 1
    if [ "$(sha256sum "$TMPDIR/geoip.dat" | awk '{print $1}')" != "$(cat "$TMPDIR/geoip.dat.sha256sum" | awk '{print $1}')" ]; then
        print_error "geoip.dat checksum error"
        rm -rf "$TMPDIR"
        exit 1
    fi

    # geosite.dat
    print_info "Downloading ${mirror}https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat"
    curl --connect-timeout 5 -m 120 $CURL_OPTS -o "$TMPDIR/geosite.dat" "${mirror}https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat"
    [ $? -ne 0 ] && rm -rf "$TMPDIR" && exit 1
    # checksum - geosite.dat
    print_info "Downloading ${mirror}https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat.sha256sum"
    curl --connect-timeout 5 -m 10 $CURL_OPTS -o "$TMPDIR/geosite.dat.sha256sum" "${mirror}https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat.sha256sum"
    [ $? -ne 0 ] && rm -rf "$TMPDIR" && exit 1
    if [ "$(sha256sum "$TMPDIR/geosite.dat" | awk '{print $1}')" != "$(cat "$TMPDIR/geosite.dat.sha256sum" | awk '{print $1}')" ]; then
        print_error "geosite.dat checksum error"
        rm -rf "$TMPDIR"
        exit 1
    fi
    rm -rf "$TMPDIR"/*.sha256sum
    \cp -a "$TMPDIR"/* /usr/share/v2ray
    rm -rf "$TMPDIR"
)

restart_service() {
    /etc/init.d/mosdns restart
}

flush_cache() {
    curl -s 127.0.0.1:$(uci -q get mosdns.config.listen_port_api)/plugins/lazy_cache/flush || exit 1
}

v2dat_dump() {
    # env
    v2dat_dir=/usr/share/v2ray
    adblock=$(uci -q get mosdns.config.adblock)
    ad_source=$(uci -q get mosdns.config.ad_source)
    configfile=$(uci -q get mosdns.config.configfile)
    streaming_media=$(uci -q get mosdns.config.custom_stream_media_dns)
    mkdir -p /var/mosdns
    rm -f /var/mosdns/geo*.txt
    if [ "$configfile" = "/var/etc/mosdns.json" ]; then
        # default config
        v2dat unpack geoip -o /var/mosdns -f cn $v2dat_dir/geoip.dat
        v2dat unpack geosite -o /var/mosdns -f cn -f apple -f 'geolocation-!cn' $v2dat_dir/geosite.dat
        [ "$adblock" -eq 1 ] && [ $(echo $ad_source | grep -c geosite.dat) -ge '1' ] && v2dat unpack geosite -o /var/mosdns -f category-ads-all $v2dat_dir/geosite.dat
        if [ "$streaming_media" = "1" ]; then
            v2dat unpack geosite -o /var/mosdns -f netflix -f disney -f hulu $v2dat_dir/geosite.dat
        else
            touch /var/mosdns/geosite_disney.txt
            touch /var/mosdns/geosite_netflix.txt
            touch /var/mosdns/geosite_hulu.txt
        fi
    else
        # custom config
        v2dat unpack geoip -o /var/mosdns -f cn $v2dat_dir/geoip.dat
        v2dat unpack geosite -o /var/mosdns -f cn -f 'geolocation-!cn' $v2dat_dir/geosite.dat
        geoip_tags=$(uci -q get mosdns.config.geoip_tags)
        geosite_tags=$(uci -q get mosdns.config.geosite_tags)
        [ -n "$geoip_tags" ] && v2dat unpack geoip -o /var/mosdns $(echo $geoip_tags | sed -r 's/\S+/-f &/g') $v2dat_dir/geoip.dat
        [ -n "$geosite_tags" ] && v2dat unpack geosite -o /var/mosdns $(echo $geosite_tags | sed -r 's/\S+/-f &/g') $v2dat_dir/geosite.dat
    fi
}

case $script_action in
    "dns")
        interface_dns
    ;;
    "adlist")
        get_adlist
    ;;
    "geodata")
        geodat_update && adlist_update && restart_service
    ;;
    "logfile")
        logfile_path
    ;;
    "adlist_update")
        adlist_update && [ "$has_update" -eq 1 ] && restart_service
    ;;
    "flush")
        flush_cache
    ;;
    "v2dat_dump")
        v2dat_dump
    ;;
    "version")
        mosdns version
    ;;
    *)
        exit 0
    ;;
esac
