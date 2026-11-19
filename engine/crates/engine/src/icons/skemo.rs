//! skemo's own icons (`skemo-*`, D-115): infrastructure things Lucide does not draw, or not as a
//! diagram needs them (a VM, a hypervisor, a rack). Drawn for the set: 24 x 24, round caps, the
//! stroke Lucide's take; a dot is filled with `currentColor`. Edited by hand; sorted by name.

/// `(name, inner SVG markup of the 24x24 icon, search tags)`, sorted by name.
pub(super) static SKEMO: [(&str, &str, &str); 36] = [
    (
        "skemo-api-gateway",
        "<rect x=\"2\" y=\"6\" width=\"20\" height=\"12\" rx=\"2\"/><path d=\"M4 15l2.5-6 2.5 6M4.9 13h3.2M11 15V9h2a1.75 1.75 0 0 1 0 3.5h-2M18 9v6M16.75 9h2.5M16.75 15h2.5\"/>",
        "api gateway apis endpoint management rest",
    ),
    (
        "skemo-cdn",
        "<circle cx=\"12\" cy=\"12\" r=\"3\"/><circle cx=\"5\" cy=\"5\" r=\"2\"/><circle cx=\"19\" cy=\"5\" r=\"2\"/><circle cx=\"5\" cy=\"19\" r=\"2\"/><circle cx=\"19\" cy=\"19\" r=\"2\"/><path d=\"M6.5 6.5 10 10M17.5 6.5l-3 3M6.5 17.5l3 -3M17.5 17.5l-3 -3\"/>",
        "cdn content delivery network edge cache cloudfront akamai",
    ),
    (
        "skemo-cluster",
        "<rect x=\"9\" y=\"2\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"2\" y=\"17\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"16\" y=\"17\" width=\"6\" height=\"5\" rx=\"1\"/><path d=\"M10.5 7 6 17M13.5 7 18 17M8 19.5h8\"/>",
        "cluster kubernetes k8s nodes openshift group",
    ),
    (
        "skemo-container",
        "<rect x=\"2\" y=\"5\" width=\"20\" height=\"14\" rx=\"2\"/><path d=\"M6 5v14M10 5v14M14 5v14M18 5v14\"/>",
        "container docker image runtime oci",
    ),
    (
        "skemo-datacenter",
        "<path d=\"M4 21V8l8 -4.5L20 8v13M2 21h20\"/><rect x=\"7.5\" y=\"9.5\" width=\"9\" height=\"4\" rx=\"1\"/><rect x=\"7.5\" y=\"15\" width=\"9\" height=\"4\" rx=\"1\"/><path d=\"M12.5 11.5h2M12.5 17h2\"/><circle cx=\"10\" cy=\"11.5\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/><circle cx=\"10\" cy=\"17\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "datacenter data center building facility on-premises",
    ),
    (
        "skemo-dns",
        "<rect x=\"2\" y=\"6\" width=\"20\" height=\"12\" rx=\"2\"/><path d=\"M4.5 9v6h.5a3 3 0 0 0 0-6ZM10 15V9l3.5 6V9M19 9.6c-.5-.4-1.1-.6-1.7-.6-1 0-1.8.6-1.8 1.5 0 1.9 3.5 1.1 3.5 3.1 0 .9-.8 1.4-1.8 1.4-.7 0-1.4-.2-1.9-.7\"/>",
        "dns domain name system resolver route53 records zone",
    ),
    (
        "skemo-ethernet-female",
        "<rect x=\"3\" y=\"3\" width=\"18\" height=\"18\" rx=\"2\"/><path d=\"M7 8h10v7.5h-3v2.5h-4v-2.5H7ZM9 10v1.5M11 10v1.5M13 10v1.5M15 10v1.5\"/>",
        "ethernet rj45 socket port female jack cable",
    ),
    (
        "skemo-function",
        "<path d=\"M7 3H6a2 2 0 0 0 -2 2v4.5L2 12l2 2.5V19a2 2 0 0 0 2 2h1M17 3h1a2 2 0 0 1 2 2v4.5l2 2.5 -2 2.5V19a2 2 0 0 1 -2 2h-1M13.5 4 9.5 12.5h5L10.5 20\"/>",
        "function serverless lambda faas cloud function",
    ),
    (
        "skemo-hypervisor",
        "<rect x=\"3\" y=\"2\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"14\" y=\"2\" width=\"7\" height=\"7\" rx=\"1\"/><path d=\"M2 12.5h20\"/><rect x=\"2\" y=\"16\" width=\"20\" height=\"6\" rx=\"1\"/><path d=\"M10 19h8\"/><circle cx=\"6\" cy=\"19\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "hypervisor virtualization host esxi kvm proxmox hyper-v vms",
    ),
    (
        "skemo-ids",
        "<path d=\"M12 2.5 20 5.5v6c0 5-3.4 8.6-8 10-4.6-1.4-8-5-8-10v-6Z\"/><path d=\"M7.5 12s1.8-3 4.5-3 4.5 3 4.5 3-1.8 3-4.5 3-4.5-3-4.5-3Z\"/><circle cx=\"12\" cy=\"12\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "ids ips intrusion detection prevention security monitoring sensor",
    ),
    (
        "skemo-ingress",
        "<path d=\"M6 3v5M6 16v5M2 12h7M9 12l5 -5h6M9 12l5 5h6M17.5 4.5 20 7l-2.5 2.5M17.5 14.5 20 17l-2.5 2.5\"/>",
        "ingress kubernetes k8s route entry http routing",
    ),
    (
        "skemo-iot",
        "<rect x=\"7\" y=\"11\" width=\"10\" height=\"10\" rx=\"2\"/><path d=\"M9 7.5a4 4 0 0 1 6 0M6 4.5a8.5 8.5 0 0 1 12 0\"/><circle cx=\"12\" cy=\"16\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "iot internet of things connected device sensor embedded",
    ),
    (
        "skemo-ip-phone",
        "<rect x=\"4\" y=\"12\" width=\"16\" height=\"9\" rx=\"2\"/><path d=\"M9 14v5M12.5 19v-5h2a1.5 1.5 0 0 1 0 3h-2\"/><path d=\"M3 7.5C3 5.5 4.5 4 7 4h10c2.5 0 4 1.5 4 3.5V8a1 1 0 0 1 -1 1h-2.5a1 1 0 0 1 -1 -1V7h-7v1a1 1 0 0 1 -1 1H4a1 1 0 0 1 -1 -1Z\"/>",
        "ip phone voip telephone sip desk phone telephony",
    ),
    (
        "skemo-k8s-service",
        "<circle cx=\"5\" cy=\"12\" r=\"3\"/><path d=\"M8 12h2.5M10.5 6.5v11M10.5 6.5H13M10.5 17.5H13\"/><rect x=\"13\" y=\"2.5\" width=\"9\" height=\"8\" rx=\"2\"/><rect x=\"13\" y=\"13.5\" width=\"9\" height=\"8\" rx=\"2\"/><path d=\"M15.5 5h4M15.5 8h4M15.5 16h4M15.5 19h4\"/>",
        "service kubernetes k8s clusterip endpoint selector",
    ),
    (
        "skemo-load-balancer",
        "<circle cx=\"4\" cy=\"12\" r=\"2\"/><path d=\"M6 12h4M10 6v12M10 6h10M10 12h10M10 18h10M18 4l2 2-2 2M18 10l2 2-2 2M18 16l2 2-2 2\"/>",
        "load balancer lb alb nlb haproxy distribution traffic network",
    ),
    (
        "skemo-mainframe",
        "<rect x=\"2\" y=\"3\" width=\"20\" height=\"18\" rx=\"2\"/><circle cx=\"12\" cy=\"7.5\" r=\"2\"/><circle cx=\"12\" cy=\"14.5\" r=\"2\"/><path d=\"M14 7.5v7M7.5 3v18M16.5 3v18M4 8h1.5M4 11h1.5M4 14h1.5M18.5 8H20M18.5 11H20M18.5 14H20M10 18.5h4\"/><circle cx=\"12\" cy=\"7.5\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/><circle cx=\"12\" cy=\"14.5\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "mainframe ibm z legacy host batch",
    ),
    (
        "skemo-nat-gateway",
        "<path d=\"M4 21V10l8 -6 8 6v11M8 15h8M13 12l3 3 -3 3\"/>",
        "nat gateway network address translation egress outbound",
    ),
    (
        "skemo-object-storage",
        "<ellipse cx=\"12\" cy=\"6\" rx=\"8\" ry=\"2.5\"/><path d=\"M4 6l2 13c0 1.5 3 2.5 6 2.5s6 -1 6 -2.5L20 6\"/>",
        "object storage s3 bucket blob minio",
    ),
    (
        "skemo-patch-panel",
        "<rect x=\"1.5\" y=\"5.5\" width=\"21\" height=\"11\" rx=\"2\"/><rect x=\"4\" y=\"8.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"8.5\" y=\"8.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"13\" y=\"8.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"17.5\" y=\"8.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"4\" y=\"12.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"8.5\" y=\"12.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"13\" y=\"12.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><rect x=\"17.5\" y=\"12.5\" width=\"2.5\" height=\"2\" rx=\"1\"/><path d=\"M5 14.5V21M14 14.5V21\"/>",
        "patch panel cabling ports rj45 wiring",
    ),
    (
        "skemo-pod",
        "<rect x=\"2\" y=\"2.5\" width=\"20\" height=\"19\" rx=\"2\"/><rect x=\"5\" y=\"6\" width=\"14\" height=\"5.5\" rx=\"1\"/><rect x=\"5\" y=\"13\" width=\"14\" height=\"5.5\" rx=\"1\"/><path d=\"M8.5 8v1.5M12 8v1.5M15.5 8v1.5M8.5 15v1.5M12 15v1.5M15.5 15v1.5\"/>",
        "pod kubernetes k8s containers workload",
    ),
    (
        "skemo-proxy",
        "<rect x=\"8.5\" y=\"4\" width=\"7\" height=\"16\" rx=\"1\"/><path d=\"M2 9h4M4.5 7.5 6 9l-1.5 1.5M6 15H2M3.5 13.5 2 15l1.5 1.5M18 9h4M20.5 7.5 22 9l-1.5 1.5M22 15h-4M19.5 13.5 18 15l1.5 1.5\"/>",
        "proxy reverse proxy forward nginx envoy squid",
    ),
    (
        "skemo-queue",
        "<rect x=\"2\" y=\"7\" width=\"5\" height=\"10\" rx=\"1\"/><rect x=\"10\" y=\"7\" width=\"5\" height=\"10\" rx=\"1\"/><path d=\"M17.5 12H22M19.5 9.5 22 12l-2.5 2.5\"/>",
        "queue message queue broker kafka rabbitmq sqs events",
    ),
    (
        "skemo-rack",
        "<rect x=\"3\" y=\"1.5\" width=\"18\" height=\"20\" rx=\"2\"/><rect x=\"5\" y=\"4\" width=\"14\" height=\"2.5\" rx=\"1\"/><rect x=\"5\" y=\"8.5\" width=\"14\" height=\"4.5\" rx=\"1\"/><path d=\"M10.5 11h6M6 21.5v1M18 21.5v1\"/><path d=\"M7 15.5v4M17 15.5v4\" stroke-dasharray=\"0 2\"/><circle cx=\"7.5\" cy=\"11\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "rack server rack cabinet bay units",
    ),
    (
        "skemo-registry",
        "<rect x=\"8\" y=\"3\" width=\"8\" height=\"8\" rx=\"2\"/><rect x=\"3\" y=\"13\" width=\"8\" height=\"8\" rx=\"2\"/><rect x=\"13\" y=\"13\" width=\"8\" height=\"8\" rx=\"2\"/><path d=\"M10.5 7h3M5.5 17h3M15.5 17h3\"/>",
        "registry container image registry docker harbor quay artifacts",
    ),
    (
        "skemo-router",
        "<circle cx=\"12\" cy=\"12\" r=\"10\"/><path d=\"M12 4v5.5M9.5 7 12 9.5 14.5 7M12 20v-5.5M9.5 17 12 14.5 14.5 17M14.5 12H20M17.5 9.5 20 12l-2.5 2.5M9.5 12H4M6.5 9.5 4 12l2.5 2.5\"/>",
        "router routing gateway network layer 3 packets",
    ),
    (
        "skemo-san",
        "<rect x=\"2\" y=\"2.5\" width=\"20\" height=\"19\" rx=\"2\"/><circle cx=\"7\" cy=\"9\" r=\"3\"/><circle cx=\"17\" cy=\"9\" r=\"3\"/><circle cx=\"9.5\" cy=\"17\" r=\"1\"/><circle cx=\"19.5\" cy=\"17\" r=\"1\"/><path d=\"M12 2.5v19M9 16 7.5 11M19 16 17.5 11\"/><circle cx=\"7\" cy=\"9\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/><circle cx=\"17\" cy=\"9\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "san storage area network storage array disks block",
    ),
    (
        "skemo-server",
        "<rect x=\"6\" y=\"2\" width=\"12\" height=\"20\" rx=\"2\"/><rect x=\"8.5\" y=\"5\" width=\"7\" height=\"3\" rx=\"1\"/><rect x=\"8.5\" y=\"10\" width=\"7\" height=\"3\" rx=\"1\"/><circle cx=\"12\" cy=\"17.5\" r=\"1.5\"/>",
        "server physical machine host bare metal compute",
    ),
    (
        "skemo-switch",
        "<rect x=\"2\" y=\"5\" width=\"20\" height=\"14\" rx=\"2\"/><path d=\"M6 9.5h10M13.5 7 16 9.5 13.5 12M18 14.5H8M10.5 12 8 14.5l2.5 2.5\"/>",
        "switch ethernet lan network layer 2 ports",
    ),
    (
        "skemo-ups",
        "<rect x=\"5\" y=\"2\" width=\"14\" height=\"20\" rx=\"2\"/><rect x=\"8\" y=\"5.5\" width=\"7\" height=\"4\" rx=\"1\"/><path d=\"M16.5 7v1M9 16c1-1.5 2-1.5 3 0s2 1.5 3 0\"/>",
        "ups uninterruptible power supply battery power",
    ),
    (
        "skemo-vdi",
        "<rect x=\"2\" y=\"3\" width=\"20\" height=\"13\" rx=\"2\" stroke-dasharray=\"2 3\"/><path d=\"M8 21h8M12 16v5\"/>",
        "vdi virtual desktop workstation remote desktop citrix",
    ),
    (
        "skemo-vm",
        "<rect x=\"2\" y=\"2\" width=\"20\" height=\"20\" rx=\"2\" stroke-dasharray=\"2 3\"/><rect x=\"6\" y=\"8\" width=\"12\" height=\"8\" rx=\"2\"/><path d=\"M12.5 12h2.5\"/><circle cx=\"9.5\" cy=\"12\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "vm virtual machine instance ec2 compute guest",
    ),
    (
        "skemo-vpc",
        "<rect x=\"2\" y=\"2\" width=\"20\" height=\"20\" rx=\"2\" stroke-dasharray=\"2 3\"/><rect x=\"8\" y=\"12\" width=\"8\" height=\"6\" rx=\"1\"/><path d=\"M10 12v-2a2 2 0 0 1 4 0v2\"/>",
        "vpc virtual private cloud private network vnet subnet cloud network isolation",
    ),
    (
        "skemo-vpn",
        "<path d=\"M4 20v-8a8 8 0 0 1 16 0v8M2 20h20\"/><rect x=\"8.5\" y=\"12\" width=\"7\" height=\"5.5\" rx=\"1\"/><path d=\"M10 12v-2a2 2 0 0 1 4 0v2\"/>",
        "vpn gateway virtual private network tunnel ipsec wireguard remote access",
    ),
    (
        "skemo-waf",
        "<path d=\"M12 2.5 20 5.5v6c0 5 -3.5 8.5 -8 10 -4.5 -1.5 -8 -5 -8 -10v-6Z\"/><path d=\"M10 9.5 7.5 12l2.5 2.5M14 9.5l2.5 2.5 -2.5 2.5\"/>",
        "waf web application firewall security shield filtering",
    ),
    (
        "skemo-wan",
        "<path d=\"M8 12a3 3 0 0 1 -0.5 -6.5a4.5 4.5 0 0 1 8.5 -1.5A4 4 0 0 1 16 12Z\"/><path d=\"M12 12v4M5 16h14\"/><rect x=\"2\" y=\"16\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"16\" y=\"16\" width=\"6\" height=\"5\" rx=\"1\"/>",
        "wan mpls wide area network sites branch offices carrier cloud link",
    ),
    (
        "skemo-web",
        "<rect x=\"2\" y=\"3\" width=\"20\" height=\"18\" rx=\"2\"/><path d=\"M2 8h20M9 11.5 6.5 14.5 9 17.5M15 11.5l2.5 3 -2.5 3M13 11l-2 7\"/><circle cx=\"5.5\" cy=\"5.5\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/><circle cx=\"8.5\" cy=\"5.5\" r=\"1\" fill=\"currentColor\" stroke=\"none\"/>",
        "web html website page browser frontend",
    ),
];
