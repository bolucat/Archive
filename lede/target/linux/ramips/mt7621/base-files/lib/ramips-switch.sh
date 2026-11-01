# MT7621 vendor switch port names, derived from each board device tree.
ramips_switch_map()
{
	case "$1" in
	h3c,tx180x-plus-uboot-mod|\
	hiwifi,hc5962-spi-breed)
		ramips_port_map="eth0=eth0 lan1=1 lan2=2 lan3=3 wan=4"
		;;
	tplink,er605-v1|\
	tplink,er605-v2)
		ramips_port_map="dsa=eth0 eth0=eth0 eth1=eth1 lan2=1 lan3=2 lan4=3 lan5=4 wan1=eth1"
		;;
	ubnt,usw-flex)
		ramips_port_map="dsa=eth0 eth0=eth0 lan1=4 lan2=3 lan3=2 lan4=1 lan5=0"
		;;
	plasmacloud,pax1800-lite)
		ramips_port_map="dsa=eth0 eth0=eth0 lan=4"
		;;
	ubnt,edgerouter-x-sfp)
		ramips_port_map="eth0=0 eth1=1 eth2=2 eth3=3 eth4=4 eth5=5"
		;;
	tplink,deco-m4r-v4)
		ramips_port_map="eth0=0 eth1=1 lan0=0 lan1=1"
		;;
	ubnt,edgerouter-x)
		ramips_port_map="eth0=eth1 eth1=1 eth2=2 eth3=3 eth4=4"
		;;
	gnubee,gb-pc2)
		ramips_port_map="eth0=eth0 eth1=eth1 ethblack=0 ethblue=4 ethyellow=eth1"
		;;
	gnubee,gb-pc1)
		ramips_port_map="eth0=eth0 eth1=eth1 ethblack=0 ethblue=eth1"
		;;
	dlink,covr-x1860-a1)
		ramips_port_map="eth0=eth0 eth1=eth1 ethernet=2 internet=0"
		;;
	tplink,eap235-wall-v1|\
	tplink,eap615-wall-v1)
		ramips_port_map="eth0=eth0 eth1=eth1 lan0=eth1 lan1=3 lan2=2 lan3=1"
		;;
	mqmaker,witi)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=0 lan2=1 lan3=2 lan4=3 wan1=4 wan2=eth1"
		;;
	adslr,g7|\
	afoundry,ew1200|\
	asus,rt-ac85u|\
	buffalo,wsr-1166dhp|\
	buffalo,wsr-600dhp|\
	cudy,wr2100|\
	cudy,x6-v1|\
	cudy,x6-v2|\
	hilink,hlk-7621a-evb|\
	huasifei,ws1208v2|\
	jcg,jhr-ac876m|\
	linksys,e5600|\
	linksys,ea6350-v4|\
	mediatek,ap-mt7621a-v60|\
	mediatek,mt7621-eval-board|\
	mofinetwork,mofi5500-5gxelte|\
	phicomm,k2p|\
	planex,vr500|\
	samknows,whitebox-v8|\
	teltonika,rutm50|\
	totolink,a7000r|\
	totolink,x5000r|\
	tplink,archer-ax21-v4|\
	tplink,archer-ax23-v1|\
	tplink,ex220-v1|\
	unielec,u7621-06-16m|\
	unielec,u7621-06-32m|\
	unielec,u7621-06-64m|\
	xiaoyu,xy-c5|\
	youku,yk-l2|\
	yuncore,g720|\
	zbtlink,zbt-we1326|\
	zbtlink,zbt-we3526|\
	zbtlink,zbt-wg108|\
	zbtlink,zbt-wg1602-16m|\
	zbtlink,zbt-wg1602-v04-16m|\
	zbtlink,zbt-wg1602-v04-32m|\
	zbtlink,zbt-wg1608-16m|\
	zbtlink,zbt-wg1608-32m)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=0 lan2=1 lan3=2 lan4=3 wan=eth1"
		;;
	alfa-network,ax1800rm|\
	confiabits,mt7621-v1|\
	gehua,ghl-r-001|\
	teltonika,rutm11|\
	xiaomi,cr660x-uboot-mod|\
	xiaomi,mi-router-cr6606|\
	xiaomi,mi-router-cr6608|\
	xiaomi,mi-router-cr6609)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=0 lan2=1 lan3=2 wan=eth1"
		;;
	renkforce,ws-wn530hp3-a)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=1 lan2=0 wan=eth1"
		;;
	asus,4g-ax56|\
	asus,rt-ac57u-v1|\
	asus,rt-ac65p|\
	asus,rt-ac85p|\
	beeline,smartbox-turbo-plus|\
	beeline,smartbox-turbo|\
	etisalat,s3|\
	iptime,a3004ns-dual|\
	iptime,a6004ns-m|\
	iptime,a6ns-m|\
	keenetic,kn-1910|\
	keenetic,kn-3010|\
	linksys,ea7300-v1|\
	linksys,ea7300-v2|\
	linksys,ea7500-v2|\
	linksys,ea8100-v1|\
	linksys,ea8100-v2|\
	rostelecom,rt-fe-1a|\
	rostelecom,rt-sf-1|\
	snr,snr-cpe-me2-lite|\
	snr,snr-cpe-me2-sfp|\
	tenbay,t-mb5eu-v01|\
	tplink,archer-c6u-v1|\
	tplink,ec330-g5u-v1|\
	unielec,u7621-01-16m|\
	youhua,wr1200js)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=1 lan2=2 lan3=3 lan4=4 wan=eth1"
		;;
	asus,rt-ax53u|\
	buffalo,wsr-2533dhpl2|\
	buffalo,wsr-2533dhpls|\
	edup,ep-rt2960s|\
	h3c,tx1800-plus|\
	h3c,tx1801-plus|\
	h3c,tx1806|\
	hiwifi,hc5962|\
	jdcloud,re-cp-02|\
	mercusys,mr70x-v1|\
	sim,simax1800u|\
	tplink,mr600-v2-eu|\
	z-router,zr-2660|\
	z-router,zr-2662)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=1 lan2=2 lan3=3 wan=eth1"
		;;
	netgear,wax202)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=1 lan2=2 lan3=4 wan=eth1"
		;;
	beeline,smartbox-giga|\
	comfast,cf-ew84|\
	jdcloud,re-sp-01b|\
	mikrotik,routerboard-m33g)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=1 lan2=2 wan=eth1"
		;;
	jcg,q20)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=1 lan2=4 wan=eth1"
		;;
	dlink,dir-x1860-b1)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=2 lan2=1 lan3=0 wan=eth1"
		;;
	elecom,wmc-x1800gst|\
	elecom,wrc-x1800gs|\
	iodata,wn-deax1800gr|\
	iptime,ax2002m|\
	lenovo,newifi-d1|\
	xiaomi,mi-router-4)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=2 lan2=1 wan=eth1"
		;;
	tplink,ex220-v2|\
	xiaomi,mi-router-ac2100|\
	xiaomi,redmi-router-ac2100)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=2 lan2=3 lan3=4 wan=eth1"
		;;
	ampedwireless,ally-r1900k)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=2 lan2=3 lan3=eth1 wan=1"
		;;
	asiarf,ap7621-nv1|\
	glinet,gl-mt1300)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=2 lan2=3 wan=eth1"
		;;
	zyxel,wap6805)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=3 lan2=2 lan3=1 lan4=0"
		;;
	asus,rt-ax54|\
	asus,rt-n56u-b1|\
	cudy,wr1300-v1|\
	cudy,wr1300-v2|\
	cudy,wr1300-v3|\
	dlink,dir-1360-a1|\
	dlink,dir-1935-a1|\
	dlink,dir-1960-a1|\
	dlink,dir-2055-a1|\
	dlink,dir-2150-a1|\
	dlink,dir-2150-r1|\
	dlink,dir-2640-a1|\
	dlink,dir-2660-a1|\
	dlink,dir-3040-a1|\
	dlink,dir-853-a1|\
	dlink,dir-853-a3|\
	dlink,dir-853-r1|\
	dlink,dir-867-a1|\
	dlink,dir-878-a1|\
	dlink,dir-878-r1|\
	dlink,dir-882-a1|\
	dlink,dir-882-r1|\
	jcg,y2|\
	netgear,r6220|\
	netgear,r6260|\
	netgear,r6350|\
	netgear,r6700-v2|\
	netgear,r6800|\
	netgear,r6850|\
	netgear,r6900-v2|\
	netgear,r7200|\
	netgear,r7450|\
	netgear,wac124|\
	netgear,wndr3700-v5|\
	netis,n6|\
	netis,wf2881|\
	raisecom,msg1500-x-00|\
	snr,snr-cpe-me1|\
	storylink,sap-g3200u3|\
	winstars,ws-wn536p3|\
	yuncore,fap640)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=3 lan2=2 lan3=1 lan4=0 wan=eth1"
		;;
	netgear,wac104)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=3 lan2=2 lan3=1 lan4=eth1"
		;;
	haier,har-20s2u1|\
	sim,simax1800t|\
	xiaomi,mi-router-3-pro|\
	zyxel,wsm20)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=3 lan2=2 lan3=1 wan=eth1"
		;;
	beeline,smartbox-flash|\
	mts,wg430223|\
	oraybox,x3a|\
	xiaomi,mi-router-3g-v2|\
	xiaomi,mi-router-4a-gigabit)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=3 lan2=2 wan=eth1"
		;;
	iptime,a3002mesh)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=3 lan2=4 wan=eth1"
		;;
	edup,ep-rt2983)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=4 lan2=2 lan3=1 wan=eth1"
		;;
	belkin,rt1800|\
	buffalo,wsr-2533dhpl|\
	dlink,dir-860l-b1|\
	edimax,ra21s|\
	edimax,rg21s|\
	elecom,wmc-c2533gst|\
	elecom,wmc-m1267gst2|\
	elecom,wrc-1167ghbk2-s|\
	elecom,wrc-1167gs2-b|\
	elecom,wrc-1167gst2|\
	elecom,wrc-1750gs|\
	elecom,wrc-1750gst2|\
	elecom,wrc-1750gsv|\
	elecom,wrc-1900gst|\
	elecom,wrc-2533ghbk-i|\
	elecom,wrc-2533ghbk2-t|\
	elecom,wrc-2533gs2|\
	elecom,wrc-2533gst|\
	elecom,wrc-2533gst2|\
	iodata,wn-ax1167gr|\
	iodata,wn-ax1167gr2|\
	iodata,wn-ax2033gr|\
	iodata,wn-ax2033gr2|\
	iodata,wn-dx1167r|\
	iodata,wn-dx1200gr|\
	iodata,wn-dx2033gr|\
	iodata,wn-gx300gr|\
	iodata,wnpr2600g|\
	iptime,a3004t|\
	iptime,a8004t|\
	iptime,ax2004m|\
	iptime,t5004|\
	linksys,e7350)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=4 lan2=3 lan3=2 lan4=1 wan=eth1"
		;;
	linksys,re6500)
		ramips_port_map="eth0=eth0 eth1=eth1 lan1=eth1 lan2=1 lan3=2 lan4=3"
		;;
	mikrotik,routerboard-760igs)
		ramips_port_map="eth0=eth0 eth1=eth1 lan2=1 lan3=2 lan4=3 lan5=4 sfp=eth1 wan=0"
		;;
	mikrotik,routerboard-750gr3)
		ramips_port_map="eth0=eth0 eth1=eth1 lan2=1 lan3=2 lan4=3 lan5=4 wan=eth1"
		;;
	cudy,c200p|\
	cudy,r700)
		ramips_port_map="eth0=eth0 eth1=eth1 lan2=3 lan3=2 lan4=1 lan5=0 wan=eth1"
		;;
	asiarf,ap7621-001)
		ramips_port_map="eth0=eth0 eth1=eth1 lan=0 wan=eth1"
		;;
	dna,valokuitu-plus-ex400|\
	genexis,pulse-ex400|\
	humax,e10|\
	openfi,5pro|\
	teltonika,rutm30|\
	wavlink,ws-wn572hp3-4g|\
	yuncore,ax820)
		ramips_port_map="eth0=eth0 eth1=eth1 lan=1 wan=eth1"
		;;
	cudy,m1300-v2|\
	keenetic,kn-3510|\
	meig,slt866)
		ramips_port_map="eth0=eth0 eth1=eth1 lan=3 wan=eth1"
		;;
	cudy,m1800)
		ramips_port_map="eth0=eth0 eth1=eth1 lan=eth1 wan=3"
		;;
	arcadyan,we420223-99)
		ramips_port_map="eth0=eth0 eth1=eth1 swp0=eth1 swp1=1"
		;;
	d-team,pbr-m1|\
	dual-q,h721|\
	firefly,firewrt|\
	gemtek,wvrtm-130acn|\
	telco-electronics,x1|\
	tplink,ex220-v1-nand|\
	wavlink,wl-wn531a6|\
	xzwifi,creativebox-v1|\
	zbtlink,zbt-wg3526-16m|\
	zbtlink,zbt-wg3526-32m)
		ramips_port_map="eth0=eth0 lan1=0 lan2=1 lan3=2 lan4=3 wan=4"
		;;
	zyxel,lte5398-m904)
		ramips_port_map="eth0=eth0 lan1=0 lan2=2"
		;;
	wavlink,ws-wn552k1-c)
		ramips_port_map="eth0=eth0 lan1=0 lan2=2 wan=4"
		;;
	wavlink,halo-base-pro)
		ramips_port_map="eth0=eth0 lan1=0 lan2=4"
		;;
	asiarf,ap7621-004-v3|\
	hanyang,hyc-g920|\
	tplink,archer-a6-v3|\
	tplink,archer-c6-v3)
		ramips_port_map="eth0=eth0 lan1=1 lan2=2 lan3=3 lan4=4 wan=0"
		;;
	tplink,tl-wpa8631p-v3)
		ramips_port_map="eth0=eth0 lan1=1 lan2=2 lan3=3 plc0=0"
		;;
	xiaomi,mi-router-4a-gigabit-v2)
		ramips_port_map="eth0=eth0 lan1=1 lan2=2 wan=3"
		;;
	elecom,wsc-x1800gs)
		ramips_port_map="eth0=eth0 lan1=2 lan2=1"
		;;
	zyxel,lte3301-plus)
		ramips_port_map="eth0=eth0 lan1=3 lan2=2 lan3=1 lan4=0"
		;;
	alfa-network,quad-e4g|\
	beeline,smartbox-pro|\
	d-team,newifi-d2|\
	dlink,dir-3060-a1|\
	mtc,wr1201|\
	wavlink,wl-wn533a8|\
	wevo,11acnas|\
	wevo,w2914ns-v2|\
	wifire,s1500-nbn|\
	zbtlink,zbt-wg2626|\
	zio,freezio)
		ramips_port_map="eth0=eth0 lan1=3 lan2=2 lan3=1 lan4=0 wan=4"
		;;
	ruijie,rg-ew1200g-pro-v1.1|\
	ruijie,rg-ew1300g-v1)
		ramips_port_map="eth0=eth0 lan1=3 lan2=2 lan3=1 wan=0"
		;;
	xiaomi,mi-router-3g)
		ramips_port_map="eth0=eth0 lan1=3 lan2=2 wan=1"
		;;
	gemtek,wvrtm-127acn)
		ramips_port_map="eth0=eth0 lan1=3 lan2=2 wan=4"
		;;
	tozed,zlt-s12-pro)
		ramips_port_map="eth0=eth0 lan1=4 lan2=2 lan3=1 wan=0"
		;;
	elecom,wmc-s1267gs2)
		ramips_port_map="eth0=eth0 lan1=4 lan2=3 lan3=2 lan4=1"
		;;
	arcadyan,we410443|\
	asus,rp-ac56|\
	dlink,dap-1620-b1|\
	dlink,dap-x1860-a1|\
	dlink,dra-1360-a1|\
	linksys,re7000|\
	mikrotik,ltap-2hnd|\
	mikrotik,routerboard-m11g|\
	netgear,eax12|\
	netgear,ex6150|\
	netgear,wax214v2|\
	tplink,eap613-v1|\
	tplink,re350-v1|\
	tplink,re500-v1|\
	tplink,re650-v1|\
	tplink,re650-v2|\
	ubnt,unifi-6-lite|\
	ubnt,unifi-flexhd|\
	ubnt,unifi-nanohd)
		ramips_port_map="eth0=eth0 lan=0"
		;;
	maginon,mc-1200ac)
		ramips_port_map="eth0=eth0 lan=0 wan=4"
		;;
	comfast,cf-ew72-v2|\
	winstars,ws-wn583a6)
		ramips_port_map="eth0=eth0 lan=1 wan=0"
		;;
	ampedwireless,ally-00x19k|\
	cudy,lt700-outdoor-v1|\
	zyxel,lte7490-m904|\
	zyxel,nr7101)
		ramips_port_map="eth0=eth0 lan=2"
		;;
	bolt,arion)
		ramips_port_map="eth0=eth0 lan=2 modem=0 wan=1"
		;;
	wavlink,wl-wn573hx1)
		ramips_port_map="eth0=eth0 lan=3"
		;;
	asus,rp-ac87|\
	cudy,ap1300-outdoor-v1|\
	edimax,re23s|\
	sercomm,na502|\
	sercomm,na502s|\
	thunder,timecloud|\
	wodesys,wd-r1802u|\
	yuncore,fap690|\
	zyxel,nwa50ax|\
	zyxel,nwa55axe)
		ramips_port_map="eth0=eth0 lan=4"
		;;
	comfast,cf-e390ax)
		ramips_port_map="eth0=eth0 lan=4 wan=0"
		;;
	esac
}

ramips_switch_interface()
{
	local role="$1" names="$2" name entry port devices=""
	for name in $names; do
		port=""
		for entry in $ramips_port_map; do
			[ "${entry%%=*}" = "$name" ] && port="${entry#*=}" && break
		done
		case "$port" in
			eth*) devices="${devices:+$devices }$port" ;;
			[0-9]*) ramips_switch_ports="$ramips_switch_ports $port:$role" ;;
			*) return 1 ;;
		esac
	done
	[ -z "$devices" ] || ucidef_set_interface "$role" ifname "$devices"
}

ramips_switch_lan_wan()
{
	ramips_switch_interface lan "$1"
	ramips_switch_interface wan "$2"
}
