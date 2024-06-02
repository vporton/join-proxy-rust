import Call "canister:call";
import Blob "mo:base/Blob";
import Bool "mo:base/Bool";
import Debug "mo:base/Debug";
import Text "mo:base/Text";

actor Test {
    // User-Agent and Accept headers are mandatory, because they are added by IC. // FIXME
    public shared func test(addHost: Bool): async Text {
        let res = await Call.callHttp({
            url = "https://ip6-localhost:8443";
            max_response_bytes = ?10_000;
            headers = if (addHost) [
                {name = "Host"; value="localhost:8081"},
                {name = "Content-Type"; value="text/plain"},
            ] else [
                {name = "Content-Type"; value="text/plain"},
            ];
            body = null;
            method = #get;
            transform = null;
        }, 9_000_000_000_000); // TODO: much too much
        let ?body = Text.decodeUtf8(Blob.fromArray(res.body)) else {
            Debug.trap("No response body.")
        };
        body;
    }
};