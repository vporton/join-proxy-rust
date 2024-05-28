import Call "canister:call";
import Blob "mo:base/Blob";
import Bool "mo:base/Bool";
import Debug "mo:base/Debug";
import Text "mo:base/Text";

actor Test {
    public shared func test(addHost: Bool): async Text {
        let res = await Call.callHttp({
            url = "http://localhost:8081/";
            max_response_bytes = ?10_000;
            headers = if (addHost) [{name = "Host"; value="localhost:8081"}] else [];
            body = null;
            method = #get;
            transform = null;
        }, 12_000_000);
        let ?body = Text.decodeUtf8(Blob.fromArray(res.body)) else {
            Debug.trap("No response body.")
        };
        body;
    }
};