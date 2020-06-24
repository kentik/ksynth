FROM debian:stable

RUN apt-get update && apt-get install -y busybox-static

RUN groupadd --system ksynth
RUN useradd --system --gid ksynth ksynth
RUN mkdir -p /var/lib/ksynth
RUN chown ksynth:ksynth /var/lib/ksynth

COPY ksynth /

RUN setcap cap_net_raw=ep /ksynth
RUN chmod  a+x            /ksynth

FROM scratch

ENV AGENT_IDENTITY=agent.id
ENV KENTIK_COMPANY=0
ENV KENTIK_REGION=US
ENV RUST_BACKTRACE=full

VOLUME /var/lib/ksynth

COPY --from=0 /bin/busybox   /bin/
COPY --from=0 /etc/group     /etc/
COPY --from=0 /etc/passwd    /etc/
COPY --from=0 /var/lib/ksynth /var/lib/
COPY --from=0 /ksynth         /

WORKDIR /var/lib/ksynth

ENTRYPOINT ["/ksynth", "agent", "-vv"]
