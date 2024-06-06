import Http "../";
import Types "../HttpTypes";
import Blob "mo:base/Blob";
import Debug "mo:base/Debug";
import Cycles "mo:base/ExperimentalCycles";
import Iter "mo:base/Iter";

actor HttpCaller {
    stable let requestsChecker = Http.newHttpRequestsChecker();

    public shared func callHttp(
        request: Http.WrappedHttpRequest,
        params: {timeout: Nat; max_response_bytes: ?Nat64; cycles: Nat}
    ): async Types.HttpResponsePayload {
        Cycles.add<system>(params.cycles);
        await* Http.checkedHttpRequestWrapped(requestsChecker, request, ?{ function = transform; context = "" }, params);
    };

    /// This function is needed even, if you use `inspect`, because
    /// `inspect` is basically a query call and query calls can be forged by a malicious replica.
    public shared func checkRequest(hash: Blob): async () {
        if (not Http.checkHttpRequest(requestsChecker, hash)) {
            Debug.trap("hacked HTTP request");
        }
    };

    public query func transform(args: Types.TransformArgs): async Types.HttpResponsePayload {
        let headers = Iter.toArray(Iter.filter(
            args.response.headers.vals(), func (h: {name: Text; value: Text}): Bool {h.name != "date"}
        ));
        {
            status = args.response.status;
            headers;
            body = args.response.body;
        };
    };

    system func inspect({
        // caller : Principal;
        // arg : Blob;
        msg : {
            #callHttp : () ->
                (Http.WrappedHttpRequest,
                {cycles : Nat; max_response_bytes : ?Nat64; timeout : Nat});
            #checkRequest : () -> Blob;
            #transform : () -> Types.TransformArgs
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