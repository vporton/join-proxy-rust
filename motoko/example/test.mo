import Http "../";
import Call "canister:call";
import Blob "mo:base/Blob";
import Debug "mo:base/Debug";
import Text "mo:base/Text";

actor Test {
    public shared func test(path: Text, arg: Text, body: Text, port: Text, noHost: Bool)
        : async (Text, [{name: Text; value: Text}])
    {
        let headers = Http.headersNew();
        if (not noHost) {
            headers.put("Host", ["local.vporton.name:8081"]); // overrides the default // TODO: shorthand for Host header
        };
        // Add arbitrary headers for testing:
        headers.put("Content-Type", ["text/plain"]);
        headers.put("X-My", ["my"]);
        let res = await Call.callHttp(
            {
                url = "https://local.vporton.name:" # port # path # "?arg=" # arg;
                headers = headers.share();
                body = Text.encodeUtf8(body);
                method = #post;
            },
            {
                max_response_bytes = ?10_000;
                cycles = 900_000_000_000; // TODO: much too much
                timeout = 60_000_000_000; // 60 sec
            },
        );
        let ?resp_body = Text.decodeUtf8(Blob.fromArray(res.body)) else {
            Debug.trap("can't decode response body.")
        };
        if (res.status != 200) {
            Debug.trap("invalid response from proxy: " # resp_body);
        };
        (resp_body, res.headers);
    };
};