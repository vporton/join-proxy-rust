import Http "../";
import Types "../HttpTypes";
import Blob "mo:base/Blob";
import Debug "mo:base/Debug";

actor HttpCaller {
    stable let requestsChecker = Http.newHttpRequestsChecker();

    let timeout = 60 * 1_000_000_000; // 1 min

    public shared func callHttp(request: Types.HttpRequestArgs): async Types.HttpResponsePayload {
        await* Http.checkedHttpRequest(requestsChecker, request, {timeout});
    };

    /// This function is needed even, if you use `inspect`, because
    /// `inspect` is basically a query call and query calls can be forged by a malicious replica.
    public shared func checkRequest(hash: Blob): async () {
        if (not Http.checkHttpRequest(requestsChecker, hash)) {
            Debug.trap("hacked HTTP request");
        }
    };

    system func inspect({
        // caller : Principal;
        // arg : Blob;
        msg : {#callHttp : () -> Types.HttpRequestArgs; #checkRequest : () -> Blob}
    }) : Bool {
        switch (msg) {
            case (#checkRequest hash) {
                Http.checkHttpRequest(requestsChecker, hash()); // FIXME: It is an update call.
            };
            case _ {
                // Should here check permissions:
                true;
            }
        };
    };
}