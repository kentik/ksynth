FROM debian:stable

RUN apt-get update && apt-get install -y busybox-static

RUN groupadd --system synag
RUN useradd --system --gid synag synag
RUN mkdir -p /var/lib/synag
RUN chown synag:synag /var/lib/synag

COPY synag /

RUN setcap cap_net_raw=ep /synag
RUN chmod  a+x            /synag

FROM scratch

ENV AGENT_IDENTITY=agent.id
ENV KENTIK_COMPANY=0
ENV KENTIK_REGION=US
ENV RUST_BACKTRACE=full

VOLUME /var/lib/synag

COPY --from=0 /bin/busybox   /bin/
COPY --from=0 /etc/group     /etc/
COPY --from=0 /etc/passwd    /etc/
COPY --from=0 /var/lib/synag /var/lib/
COPY --from=0 /synag         /

WORKDIR /var/lib/synag

ENTRYPOINT ["/synag", "agent", "-vv"]
