import Call "canister:call";
import Bool "mo:base/Bool";

actor Test {
    public shared func test(addHost: Bool) {
        await Call.callHttp({
            url = "http://localhost:8081/";
            max_response_bytes = ?10_000;
            headers = if (addHost) [{name = "Host"; value="localhost:8081"}] else [];
            body = null;
            method = #get;
            transform = null;
        })
    }
};