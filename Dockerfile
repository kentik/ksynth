FROM debian:stable

ARG TARGETARCH
ARG TARGETPLATFORM
ARG TARGETVARIANT
ARG BINARY=binary/${TARGETARCH}${TARGETVARIANT}/ksynth

RUN apt-get update && apt-get install -y busybox-static ca-certificates libcap2-bin

RUN mkdir -p /var/lib/ksynth
RUN mkdir -p /opt/kentik/ksynth

ADD $BINARY                /opt/kentik/ksynth/
RUN setcap cap_net_raw=eip /opt/kentik/ksynth/ksynth
RUN chmod  a+x             /opt/kentik/ksynth/ksynth

FROM scratch

ENV AGENT_IDENTITY=agent.id
ENV KENTIK_COMPANY=0
ENV KENTIK_REGION=US
ENV RUST_BACKTRACE=full

VOLUME /var/lib/ksynth

COPY --from=0 /bin/busybox    /bin/
COPY --from=0 /etc/group      /etc/
COPY --from=0 /etc/passwd     /etc/
COPY --from=0 /etc/ssl        /etc/ssl
COPY --from=0 /var/lib/ksynth /var/lib/
COPY --from=0 /opt/kentik     /opt/kentik

WORKDIR /var/lib/ksynth

ENTRYPOINT ["/opt/kentik/ksynth/ksynth", "agent"]
