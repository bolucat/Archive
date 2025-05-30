FROM alpine:latest as builder
ARG TARGETPLATFORM
RUN echo "I'm building for $TARGETPLATFORM"

RUN apk add --no-cache gzip && \
    mkdir /clash-config && \
    wget -O /clash-config/geoip.metadb https://fastly.jsdelivr.net/gh/MerlinKodo/meta-rules-dat@release/geoip.metadb && \
    wget -O /clash-config/geosite.dat https://fastly.jsdelivr.net/gh/MerlinKodo/meta-rules-dat@release/geosite.dat && \
    wget -O /clash-config/geoip.dat https://fastly.jsdelivr.net/gh/MerlinKodo/meta-rules-dat@release/geoip.dat

COPY docker/file-name.sh /clash/file-name.sh
WORKDIR /clash
COPY bin/ bin/
RUN FILE_NAME=`sh file-name.sh` && echo $FILE_NAME && \
    FILE_NAME=`ls bin/ | egrep "$FILE_NAME.*"|awk NR==1` && echo $FILE_NAME && \
    mv bin/$FILE_NAME clash.gz && gzip -d clash.gz && echo "$FILE_NAME" > /clash-config/test
FROM alpine:latest
LABEL org.opencontainers.image.source="https://github.com/MerlinKodo/clash-rev"

RUN apk add --no-cache ca-certificates tzdata iptables

VOLUME ["/root/.config/clash/"]

COPY --from=builder /clash-config/ /root/.config/clash/
COPY --from=builder /clash/clash /clash
RUN chmod +x /clash
ENTRYPOINT [ "/clash" ]
