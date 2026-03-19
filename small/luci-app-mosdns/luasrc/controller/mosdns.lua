local sys  = require "luci.sys"
local http = require "luci.http"
local util = require "luci.util"

module("luci.controller.mosdns", package.seeall)

function index()
    if not nixio.fs.access("/etc/config/mosdns") then
        return
    end

    local page = entry({"admin", "services", "mosdns"}, alias("admin", "services", "mosdns", "basic"), _("MosDNS"), 30)
    page.dependent = true
    page.acl_depends = { "luci-app-mosdns" }

    entry({"admin", "services", "mosdns", "basic"}, cbi("mosdns/basic"), _("Basic Setting"), 1).leaf = true
    entry({"admin", "services", "mosdns", "rule_list"}, cbi("mosdns/rule_list"), _("Rule List"), 2).leaf = true
    entry({"admin", "services", "mosdns", "update"}, cbi("mosdns/update"), _("Geodata Update"), 3).leaf = true
    entry({"admin", "services", "mosdns", "log"}, cbi("mosdns/log"), _("Logs"), 4).leaf = true
    entry({"admin", "services", "mosdns", "status"}, call("act_status")).leaf = true
    entry({"admin", "services", "mosdns", "get_log"}, call("get_log")).leaf = true
    entry({"admin", "services", "mosdns", "clear_log"}, call("clear_log")).leaf = true
    entry({"admin", "services", "mosdns", "geo_update"}, call("geo_update")).leaf = true
    entry({"admin", "services", "mosdns", "flush_cache"}, call("flush_cache")).leaf = true
end

function act_status()
    local e = {}
    e.running = sys.call("pgrep -x mosdns >/dev/null") == 0
    http.prepare_content("application/json")
    http.write_json(e)
end

function get_log()
    local logfile = sys.exec("/usr/share/mosdns/mosdns.sh logfile"):gsub("%s+$", "")
    if logfile == "" or not logfile:match("^/") then
        http.prepare_content("text/plain")
        http.write("Log file not configured")
        return
    end
    local log = sys.exec("cat " .. util.shellquote(logfile))
    http.prepare_content("text/plain")
    http.write(log)
end

function clear_log()
    local logfile = sys.exec("/usr/share/mosdns/mosdns.sh logfile"):gsub("%s+$", "")
    if logfile == "" or not logfile:match("^/") then
        return
    end
    sys.call("cat /dev/null > " .. util.shellquote(logfile))
end

function geo_update()
    local e = {}
    e.updating = sys.call("/usr/share/mosdns/mosdns.sh geodata >/dev/null") == 0
    http.prepare_content("application/json")
    http.write_json(e)
end

function flush_cache()
    local e = {}
    e.flushing = sys.call("/usr/share/mosdns/mosdns.sh flush >/dev/null") == 0
    http.prepare_content("application/json")
    http.write_json(e)
end
