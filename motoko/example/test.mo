import Http "../";
import Call "canister:call";
import Blob "mo:base/Blob";
import Bool "mo:base/Bool";
import Debug "mo:base/Debug";
import Text "mo:base/Text";

actor Test {
    public shared func test(addHost: Bool): async Text {
        let headers = Http.headersNew();
        // Add arbitrary headers for testing:
        headers.put("Content-Type", "text/plain");
        headers.put("X-My", "my");
        if (addHost) {
            headers.put("Host", "local.vporton.name:8081"); // overrides the default
        };
        let res = await Call.callHttp({
            url = "https://local.vporton.name:8443";
            max_response_bytes = ?10_000;
            headers;
            body = null;
            method = #get;
            transform = null;
        }, 900_000_000_000); // TODO: much too much
        let ?body = Text.decodeUtf8(Blob.fromArray(res.body)) else {
            Debug.trap("No response body.")
        };
        body;
    }
};