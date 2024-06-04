import Http "../";
import Types "../HttpTypes";
import Blob "mo:base/Blob";
import Debug "mo:base/Debug";
import Cycles "mo:base/ExperimentalCycles";

actor HttpCaller {
    stable let requestsChecker = Http.newHttpRequestsChecker();

    public shared func callHttp(
        request: Http.WrappedHttpRequest,
        transform: ?Types.TransformRawResponseFunction,
        params: {timeout: Nat; max_response_bytes: ?Nat64; cycles: Nat}
    ): async Types.HttpResponsePayload {
        Cycles.add<system>(params.cycles);
        await* Http.checkedHttpRequestWrapped(requestsChecker, request, transform, params);
    };

    /// This function is needed even, if you use `inspect`, because
    /// `inspect` is basically a query call and query calls can be forged by a malicious replica.
    public shared func checkRequest(hash: Blob): async () {
        if (not Http.checkHttpRequest(requestsChecker, hash)) {
            Debug.trap("hacked HTTP request");
        }
    };

    system func inspect({
        caller : Principal;
        arg : Blob;
        msg : {
            #callHttp : () ->
                (Http.WrappedHttpRequest, ?Types.TransformRawResponseFunction,
                {cycles : Nat; max_response_bytes : ?Nat64; timeout : Nat});
            #checkRequest : () -> Blob;
        }
    }) : Bool {
        switch (msg) {
            case (#checkRequest hash) {
                Http.checkHttpRequest(requestsChecker, hash());
            };
            case _ {
                // Should here check permissions:
                true;
            }
        };
    };
}