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
  //     { id: UiView.typeId },
  //   ],
  // }))
};

function doRequest(serializedPowerboxDescriptor) {
  sendRpc("powerboxRequest", {
    query: [serializedPowerboxDescriptor]
  }).then((response) => {
    console.log("response: " + JSON.stringify(response));

    if (response.canceled) {
      console.log("canceled");
    } else {
      return http("/token/" + response.token, "post", response.descriptor).then((response) => {
        console.log("OK");
      });
    }
  });
}


class AddGrain extends React.Component {
  props: {};
  state: {};

  constructor(props) {
    super(props);
  }

  handleClick(event) {
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
           viewInfos: Immutable.Map,
           canWrite: bool,
         };
  state: { selectedGrains: Immutable.Set,
           searchString: String,
         };

  constructor(props) {
    super(props);
    this.state = { selectedGrains: Immutable.Set(),
                   searchString: "",
                 };

    this._currentlyRendered = {};
  }

  clickRemoveGrain(e) {
    let newSelected = this.state.selectedGrains;

    for (let e of this.state.selectedGrains.keys()) {
      if (e in this._currentlyRendered) {
        http("/sturdyref/" + e, "delete");
        newSelected = newSelected.remove(e);
      }
    }

    this.setState({ selectedGrains: newSelected });
  }

  selectGrain(e) {
    const token = e.target.getAttribute("data-token");
    if (this.state.selectedGrains.get(token)) {
      this.setState({ selectedGrains: this.state.selectedGrains.remove(token) });
    } else {
      this.setState({ selectedGrains: this.state.selectedGrains.add(token) });
    }
  }

  clickCheckboxContainer(e) {
    if (e.target.tagName === "TD") {
      for (let ii = 0; ii < e.target.children.length; ++ii) {
        const c = e.target.children[ii];
        if (c.tagName === "INPUT") {
          c.click();
          return;
        }
      }
    }
  }

  selectAll(e) {
    if (!e.target.checked) {
      let newSelected = this.state.selectedGrains;
      for (let e of this.state.selectedGrains.keys()) {
        if (e in this._currentlyRendered) {
          newSelected = newSelected.remove(e);
        }
      }

      this.setState({ selectedGrains: newSelected });
    } else {
      let newSelected = this.state.selectedGrains;
      for (const e in this._currentlyRendered) {
        newSelected = newSelected.add(e);
      }

      this.setState({ selectedGrains: newSelected });
    }
  }

  offerUiView(token) {
    console.log("offering token:", token);
    http("/offer/" + token, "post");
  }

  searchStringChange(e) {
    this.setState({ searchString: e.target.value});
  }

  matchesAppOrGrainTitle = function (needle, grain, info) {
    if (grain && grain.title && grain.title.toLowerCase().indexOf(needle) !== -1) return true;
    if (info && info.appTitle && info.appTitle.toLowerCase().indexOf(needle) !== -1) return true;
    return false;
  }

  render() {
    const searchKeys = this.state.searchString.toLowerCase()
          .split(" ")
          .filter((k) => k !== "");

    const matchFilter = (grain, info) => {
      if (searchKeys.length == 0) {
        return true;
      } else {
        return _.chain(searchKeys)
          .map((sk) => this.matchesAppOrGrainTitle(sk, grain, info))
          .reduce((a,b) => a && b)
          .value();
      }
    };


    let numShownAndSelected = 0;
    this._currentlyRendered = {};
    const grainRows = [];
    for (let e of this.props.grains.entries()) {
      const grain = e[1];
      const info = this.props.viewInfos.get(e[0]) || {};
      if (matchFilter(grain, info)) {
        if (this.state.selectedGrains.get(e[0])) {
          numShownAndSelected += 1;
        }
        this._currentlyRendered[e[0]] = true;

        grainRows.push(
            <tr className="grain" key={e[0]} data-token={e[0]}>
          { this.props.canWrite ?
            <td onClick={this.clickCheckboxContainer.bind(this)}>
            <input data-token={e[0]} type="checkbox" checked={!!this.state.selectedGrains.get(e[0])}
                   onChange={this.selectGrain.bind(this)}/>
            </td> :
            [] }
          <td>
          <img title={info.appTitle} src={info.grainIconUrl} className="grain-icon"></img>
          </td>
          <td className="click-to-go" onClick={this.offerUiView.bind(this, e[0])}>
          {e[1].title}
        </td>
          <td> {makeDateString(new Date(parseInt(grain.dateAdded)))}</td>
          <td> {grain.addedBy}</td>
          </tr>
        );
      }
    }

    const bulkActionButtons = [];
    if (this.props.canWrite) {
      bulkActionButtons.push(
          <button key="unlink"
                  disabled={numShownAndSelected==0}
                 title={numShownAndSelected==0 ?
                        "select grains to unlink them" : "unlink selected grains"}
                  onClick={this.clickRemoveGrain.bind(this)}>Unlink from collection... </button>);
    }

    return <div className="grain-list">
      <div className="search-row">
      <label>
      <input className="search-bar" type="text" placeholder="search"
             onChange={this.searchStringChange.bind(this)}/>
      </label>
      </div>
      <div className="bulk-action-buttons">
      {bulkActionButtons}
    </div>
      <div className="buttons"> {this.props.canWrite ? <AddGrain/>: [] } </div>

      <table className="grain-list-table">
          <thead>
           <tr>
         {this.props.canWrite ?
            <td onClick={this.clickCheckboxContainer.bind(this)}
              className="select-all-grains">
          <input type="checkbox" onChange={this.selectAll.bind(this)}
                 checked={numShownAndSelected > 0}/>
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
        <button className="description-button">done</button>
        </form>;
    } else {
      let button = [];
      if (this.props.canWrite) {
        button = <button className="description-button"
                         onClick={this.clickEdit.bind(this)}>edit</button>;
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
           viewInfos: Immutable.Map,
           socketReadyState: String,
         };

  constructor(props) {
    super(props);
    this.state = { grains: Immutable.Map(),
                   viewInfos: Immutable.Map(),
                 };
  }

  componentDidMount() {
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
        const newGrains = this.state.grains.set(action.insert.token,
                                                action.insert.data);
        this.setState({grains: newGrains});
      } else if (action.remove) {
        const newGrains = this.state.grains.delete(action.remove.token);
        this.setState({ grains: newGrains });
      } else if (action.viewInfo) {
        const newViewInfos = this.state.viewInfos.set(action.viewInfo.token,
                                                      action.viewInfo.data);
        this.setState({ viewInfos: newViewInfos });
      }
    };

  }

  render() {

    return <div>
      <p>socket state: {this.state.socketReadyState}</p>
      <Description canWrite={this.state.canWrite} description={this.state.description}/>
      <GrainList grains={this.state.grains} viewInfos={this.state.viewInfos}
                 canWrite={this.state.canWrite}/>
      </div>;
  }
}

ReactDOM.render(<Main/>,  document.getElementById("main"));


