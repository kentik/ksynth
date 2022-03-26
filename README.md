# ksynth - kentik synthetic agent

Ksynth is Kentik's lightweight, high-performance, synthetic monitoring
agent. Ksynth monitors network performance with classic ICMP ping;
traceroute using ICMP, TCP, or UDP packets; and a diagnostic called
knock which performs a partial TCP handshake. Ksynth is also capable
of application availability & performance monitoring via HTTP/1.1 &
HTTP/2 requests, DNS queries, and TLS handshakes.

Ksynth is written in async Rust and can efficiently execute thousands
of synthetic monitoring tasks on a single node with minimal resource
utilization.
