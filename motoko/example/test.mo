import Http "../";
import Call "canister:call";
import Blob "mo:base/Blob";
import Debug "mo:base/Debug";
import Text "mo:base/Text";

actor Test {
    // TODO: Remove `noHost`.
    public shared func test(path: Text, arg: Text, body: Text, port: Text)
        : async (Text, [{name: Text; value: Text}])
    {
        // Remark: As test_port_443 test shows, port is included in default Host: iff it is included in the URL.
        let headers = Http.headersNew("local.vporton.name:8081");
        // Add arbitrary headers for testing:
        headers.put("Content-Type", ["text/plain"]);
        headers.put("X-My", ["my"]);
        let res = await Call.callHttp(
            {
                url = "https://local.vporton.name" # port # path # "?arg=" # arg;
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