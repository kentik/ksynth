@0xc75f49ee0059f55d;

struct Custom {
       id @0: UInt32;
       value: union {
             uint32Val @1: UInt32;
             float32Val @2: Float32;
             strVal @3: Text;
             uint64Val @5: UInt64;
             addrVal @6: Data;
             uint16Val @7: UInt16;
             uint8Val @8: UInt8;
       }
       isDimension @4: Bool;
}

struct CHF {
        timestampNano @0: Int64;
        dstAs @1: UInt32;
        dstGeo @2: UInt32;
        dstMac @3: UInt32;          # Ignore, replaced with dstEthMac
        headerLen @4: UInt32;
        inBytes @5: UInt64;
        inPkts @6: UInt64;
        inputPort @7: UInt32;
        ipSize @8: UInt32;
        ipv4DstAddr @9: UInt32;
        ipv4SrcAddr @10: UInt32;
        l4DstPort @11: UInt32;
        l4SrcPort @12: UInt32;
        outputPort @13: UInt32;
        protocol @14: UInt32;
        sampledPacketSize @15: UInt32;
        srcAs @16: UInt32;
        srcGeo @17: UInt32;
        srcMac @18: UInt32;         # Ignore, replaced with srcEthMac
        tcpFlags @19: UInt32;
        tos @20: UInt32;
        vlanIn @21: UInt32;
        vlanOut @22: UInt32;
        ipv4NextHop @23: UInt32;
        mplsType @24: UInt32;
        outBytes @25: UInt64;
        outPkts @26: UInt64;
        tcpRetransmit @27: UInt32;
        srcFlowTags @28: Text;
        dstFlowTags @29: Text;
        sampleRate @30: UInt32;
        deviceId @31: UInt32;
        flowTags @32: Text;
        timestamp @33: Int64;
        dstBgpAsPath @34: Text;
        dstBgpCommunity @35: Text;
        srcBgpAsPath @36: Text;
        srcBgpCommunity @37: Text;
        srcNextHopAs @38: UInt32;
        dstNextHopAs @39: UInt32;
        srcGeoRegion @40: UInt32;
        dstGeoRegion @41: UInt32;
        srcGeoCity @42: UInt32;
        dstGeoCity @43: UInt32;
        big @44: Bool;
        sampleAdj @45: Bool;
        ipv4DstNextHop @46: UInt32;
        ipv4SrcNextHop @47: UInt32;
        srcRoutePrefix @48: UInt32;
        dstRoutePrefix @49: UInt32;
        srcRouteLength @50: UInt8;
        dstRouteLength @51: UInt8;
        srcSecondAsn @52: UInt32;
        dstSecondAsn @53: UInt32;
        srcThirdAsn @54: UInt32;
        dstThirdAsn @55: UInt32;
        ipv6DstAddr @56: Data;
        ipv6SrcAddr @57: Data;
        srcEthMac   @58: UInt64;
        dstEthMac   @59: UInt64;
        custom      @60: List(Custom);
        ipv6SrcNextHop @61: Data;
        ipv6DstNextHop @62: Data;
        ipv6SrcRoutePrefix @63: Data;
        ipv6DstRoutePrefix @64: Data;
        isMetric @65: Bool;
}

struct PackedCHF {
        msgs  @0: List(CHF);
}
