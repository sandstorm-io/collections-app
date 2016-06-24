import "babel-polyfill";
import React from "react";
import ReactDOM from "react-dom";
import Immutable from "immutable";

function Delete() {}

function http(url: string, postData?: string | Blob | Delete): Promise<string> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.onload = () => {
      if (xhr.status >= 400) {
        reject(new Error("XHR returned status " + xhr.status + ":\n" + xhr.responseText));
      } else {
        resolve(xhr.responseText);
      }
    };
    xhr.onerror = (e: Error) => { reject(e); };
    if (postData instanceof Delete) {
      xhr.open("delete", url)
      xhr.send();
    } else {
      xhr.open(postData ? "post" : "get", url)
      xhr.send(postData);
    }
  });
}

let rpcCounter = 0;
const rpcs: { [key: number]: (response: mixed) => void } = {};

window.addEventListener("message", (event) => {
  if (event.source !== window.parent ||
      typeof event.data !== "object" ||
      typeof event.data.rpcId !== "number") {
    console.warn("got unexpected postMessage:", event);
    return;
  }

  const handler = rpcs[event.data.rpcId];
  if (!handler) {
    console.error("no such rpc ID for event", event);
    return;
  }

  delete rpcs[event.data.rpcId];
  handler(event.data);
});

function sendRpc(name: string, message: Object): Promise<any> {
  const id = rpcCounter++;
  message.rpcId = id;
  const obj = {};
  obj[name] = message;
  window.parent.postMessage(obj, "*");
  return new Promise((resolve, reject) => {
    rpcs[id] = (response) => {
      if (response.error) {
        reject(new Error(response.error));
      } else {
        resolve(response);
      }
    };
  });
}

const interfaces = {
  // Powerbox descriptors for various interface IDs.

  uiView: "EAZQAQEAABEBF1EEAQH_5-Jn6pjXtNsAAAA", // 15831515641881813735,
  // This is produced by:
  // urlsafeBase64(capnp.serializePacked(PowerboxDescriptor, {
  //   tags: [
  //     { id: IpNetwork.typeId },
  //   ],
  // }))
};

function doRequest(serializedPowerboxDescriptor) {
  sendRpc("powerboxRequest", {
    query: [serializedPowerboxDescriptor]
  }).then((response) => {
    console.log("response: " + JSON.stringify(response));
    return http("/token/" + response.token, response.descriptor).then((response) => {
      console.log("OK");
    });
  });
}


class AddGrain extends React.Component {
  props: {};
  state: {};

  constructor(props) {
    super(props);
  }

  handleClick(event) {
    console.log("clicked add grain");
    doRequest(interfaces.uiView);
  }

  render() {
    return <button onClick={this.handleClick}> Add grain... </button>;
  }
}

class GrainList extends React.Component {
  props: {};
  state: { grains: Immutable.Map};

  constructor(props) {
    super(props);
    this.state = { grains: Immutable.Map() };


    let wsProtocol = window.location.protocol == "http:" ? "ws" : "wss";
    let ws = new WebSocket(wsProtocol + "://" + window.location.host);

    // TODO: error handling / reconnect

    ws.onmessage = (m) => {
      console.log("websocket got message: ", m.data);
      const action = JSON.parse(m.data);
      if (action.insert) {
        console.log("insert!", action.insert);
        const newGrains = this.state.grains.set(action.insert.token,
                                                action.insert.data);
        this.setState({grains: newGrains});
      } else if (action.remove) {
        console.log("remove!");
      }
    }

  }

  render () {
    const grainRows = [];
    for (let e of this.state.grains.entries()) {
      grainRows.push(
          <tr classNamme="grain">
          <td>
          </td>
          <td>
          {e[1].title}
          </td>
          </tr>
      );
    }

    return <div className="grain-list">
        <table className="grain-list-table">
          <thead>
            <tr>
              <td className="select-all-grains">
                <input type="checkbox"/>
              </td>
              <td className="td-app-icon"></td>
              <td className="grain-name">Name</td>
              <td className="last-used">Last activity</td>
              <td className="shared-or-owned">Mine/Shared</td>
            </tr>
          </thead>
        <tbody>
      { grainRows }
    </tbody>
    </table>
      </div>;
  }
}

ReactDOM.render(
      <div><h1>Collections</h1>
      <AddGrain/>
      <GrainList/>
  </div>,
  document.getElementById("main")
);


