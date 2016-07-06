import "babel-polyfill";
import React from "react";
import ReactDOM from "react-dom";
import Immutable from "immutable";
import _ from "underscore";

function http(url: string, method, data): Promise<string> {
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
    xhr.open(method, url);
    xhr.send(data);
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
    return http("/token/" + response.token, "post", response.descriptor).then((response) => {
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
  props: { grains: Immutable.Map,
           canWrite: bool,
         };
  state: { selectedGrains: Immutable.Set };

  constructor(props) {
    super(props);
    this.state = { selectedGrains: Immutable.Set() };
  }

  clickRemoveGrain(e) {
    for (let e of this.state.selectedGrains.keys()) {
      http("/sturdyref/" + e, "delete");
    }

    this.setState({ selectedGrains: Immutable.Set() });

  }

  selectGrain(e) {
    const token = e.target.getAttribute("data-token");
    console.log("select grain", token);
    if (e.target.checked) {
      this.setState({ selectedGrains: this.state.selectedGrains.add(token) });
    } else {
      this.setState({ selectedGrains: this.state.selectedGrains.remove(token) });
    }
  }

  offerUiView(token) {
    console.log("offering token:", token);
    http("/offer/" + token, "post");
  }

  render() {
    const grainRows = [];
    for (let e of this.props.grains.entries()) {
      grainRows.push(
          <tr className="grain" key={e[0]}>
          { this.props.canWrite ?
            <td><input data-token={e[0]} type="checkbox" onChange={this.selectGrain.bind(this)}/>
            </td> :
            [] }
          <td></td>
          <td className="click-to-go" onClick={this.offerUiView.bind(this, e[0])}>
          {e[1].title}
        </td>
          <td> {makeDateString(new Date(parseInt(e[1].date_added)))}</td>
          <td> {e[1].added_by}</td>
          </tr>
      );
    }

    const bulkActionButtons = [];
    if (this.props.canWrite) {
      bulkActionButtons.push(
          <button key="unlink"
                  onClick={this.clickRemoveGrain.bind(this)}>Unlink from collection... </button>);
    }

    return <div className="grain-list">
      {bulkActionButtons}
      <table className="grain-list-table">
          <thead>
           <tr>
         {this.props.canWrite ?
              <td className="select-all-grains">
                <input type="checkbox"/>
       </td> : [] }
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

class Description extends React.Component {
  props: { description: String, canWrite: bool };
  state: { editing: bool, editedDescription: String };

  constructor(props) {
    super(props);
    this.state = { editing: false };
  }

  clickEdit() {
    this.setState({ editing: true, editedDescription: this.props.description });
  }

  submitEdit(e) {
    e.preventDefault();
    http("/description", "put", this.state.editedDescription);
    this.setState({ editing: false });
  }

  changeDesc(e) {
    this.setState({ editedDescription: e.target.value });
  }

  render () {
    if (this.state.editing) {
      return <form onSubmit={this.submitEdit.bind(this)}>
        <input type="text" onChange={this.changeDesc.bind(this)}
               defaultValue={this.props.description}>
        </input>
        <button>done</button>
        </form>;
    } else {
      let button = [];
      if (this.props.canWrite) {
        button = <button key="hi" onClick={this.clickEdit.bind(this)}>edit</button>;
      }
      return <p>{this.props.description} {button}</p>;
    }
  }
}

class Main extends React.Component {
  props: {};
  state: { canWrite: bool,
           description: String,
           grains: Immutable.Map,
           socketReadyState: String,
         };

  constructor(props) {
    super(props);
    this.state = { grains: Immutable.Map() };
    this.openWebSocket(0);
  }

  openWebSocket(delay) {
    this.setState({socketReadyState: "connecting" });

    let wsProtocol = window.location.protocol == "http:" ? "ws" : "wss";
    let ws = new WebSocket(wsProtocol + "://" + window.location.host);

    ws.onopen = (e) => {
      this.setState({ socketReadyState: "open" });
    };

    ws.onerror = (e) => {
      console.log("websocket got error: ", e);
    };

    ws.onclose = (e) => {
      console.log("websocket closed: ", e);
      this.setState({ socketReadyState: "closed" });

      // TODO delay
      this.openWebSocket(0);
    };

    ws.onmessage = (m) => {
      console.log("websocket got message: ", m.data);
      const action = JSON.parse(m.data);
      if (action.canWrite) {
        this.setState({canWrite: action.canWrite});
      } else if (action.description) {
        this.setState({ description: action.description });
      } else if (action.insert) {
        console.log("insert!", action.insert);
        const newGrains = this.state.grains.set(action.insert.token,
                                                action.insert.data);
        this.setState({grains: newGrains});
      } else if (action.remove) {
        console.log("remove! ", action.remove.token);
        const newGrains = this.state.grains.delete(action.remove.token);
        this.setState({grains: newGrains});
      }
    };

  }

  render() {

    return <div>
      <p>socket state: {this.state.socketReadyState}</p>
      <Description canWrite={this.state.canWrite} description={this.state.description}/>
      {this.state.canWrite ? <AddGrain/>: [] }
      <GrainList grains={this.state.grains} canWrite={this.state.canWrite}/>
      </div>;
  }
}

ReactDOM.render(<Main/>,  document.getElementById("main"));


