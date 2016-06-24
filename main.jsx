import "babel-polyfill";
import React from "react";
import ReactDOM from "react-dom";
import Immutable from "immutable";
import _ from "underscore";

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

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
function makeDateString(date) {
  if (!date) {
    return "";
  }

  let result;

  const now = new Date();
  const diff = now.valueOf() - date.valueOf();

  if (diff < 86400000 && now.getDate() === date.getDate()) {
    result = date.toLocaleTimeString();
  } else {
    result = MONTHS[date.getMonth()] + " " + date.getDate() + " ";

    if (now.getFullYear() !== date.getFullYear()) {
      result = date.getFullYear() + " " + result;
    }
  }

  return result;
};

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
        console.log("remove! ", action.remove.token);
        const newGrains = this.state.grains.delete(action.remove.token);
        this.setState({grains: newGrains});
      }
    }

  }

  clickRemoveGrain(e) {
    for (let e of this.state.grains.entries()) {
      if (e[1].checked) {
        http("/sturdyref/" + e[0], new Delete())
      }
    }
  }

  selectGrain(e) {
    const token = e.target.getAttribute("data-token");
    console.log("select grain", token);

    const oldValue = this.state.grains.get(token);
    const newValue = _.clone(oldValue);
    newValue.checked = !oldValue.checked;
    this.setState({grains: this.state.grains.set(token, newValue)});

  }

  render() {
    const grainRows = [];
    for (let e of this.state.grains.entries()) {
      grainRows.push(
          <tr classNamme="grain" key={e[0]}>
          <td><input data-token={e[0]} type="checkbox" onChange={this.selectGrain.bind(this)}/>
          </td>
          <td></td>
          <td>
          {e[1].title}
        </td>
          <td> {makeDateString(new Date(parseInt(e[1].date_added)))}</td>
          <td> {e[1].added_by}</td>
          </tr>
      );
    }

    return <div className="grain-list">
      <button onClick={this.clickRemoveGrain.bind(this)}> Unlink from collection... </button>
        <table className="grain-list-table">
          <thead>
            <tr>
              <td className="select-all-grains">
                <input type="checkbox"/>
              </td>
              <td className="td-app-icon"></td>
              <td className="grain-name">Name</td>
              <td className="date-added">Date added</td>
              <td className="added-by">Added by</td>
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
    <div><p>short editable description</p>
    <AddGrain/>
    <GrainList/>
  </div>,
  document.getElementById("main")
);


