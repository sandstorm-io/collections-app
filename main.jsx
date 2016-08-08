import "babel-polyfill";
import React from "react";
import ReactDOM from "react-dom";
import Immutable from "immutable";
import _ from "underscore";

function http(url: string, method, data): Promise<string> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    if (method === "delete") {
      // Work around Firefox bug: https://bugzilla.mozilla.org/show_bug.cgi?id=521301
      xhr.responseType = "text";
    }

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
    if (response.canceled) {
      console.log("powerbox request was canceled");
    } else {
      return http("/token/" + response.token, "post", response.descriptor).then((response) => {
      });
    }
  });
}

// Icons borrowed from the main Sandstorm repo.

const SEARCH_ICON = <svg className="search-icon" version="1.1" viewBox="-7 166 20 20">
      <path d="M10.9,182.9l-5.1-5.6c0.9-1.1,1.4-2.4,1.4-4c0-3.4-2.8-6.2-6.2-6.2s-6.2,2.8-6.2,6.2c0,3.4,2.8,6.2,6.2,6.2 c1.2,0,2.4-0.4,3.4-1l5.1,5.6c0.4,0.4,0.9,0.4,1.3,0.1C11.2,183.9,11.2,183.3,10.9,182.9z M-2.1,176.5c-0.8-0.8-1.3-1.9-1.3-3.1 c0-1.2,0.5-2.3,1.3-3.2c0.8-0.8,1.9-1.3,3.2-1.3c1.2,0,2.3,0.5,3.2,1.3c0.7,0.8,1.2,1.9,1.2,3.2c0,1.2-0.5,2.3-1.3,3.2 c-0.8,0.8-1.9,1.3-3.2,1.3C-0.2,177.8-1.3,177.3-2.1,176.5z"/>
      </svg>;

const INSTALL_ICON = <svg version="1.1" viewBox="32 32 64 64" >
	  <path d="M58.8,71.2H37.5V58.6h21.3V36.4h13v22.2h21.3v12.6H71.8v22.2h-13V71.2z"/>
      </svg>;

const EDIT_ICON = <svg version="1.1" viewBox="-4.5 168.5 15 15">
	  <polygon points="1.1,179.6 -0.9,180.1 -0.5,178.1 	"/>
	  <path d="M6.4,171.5c-0.2-0.2-0.6-0.1-0.8,0.1l-5.8,6.1l1.7,1.7l5.8-6.1c0.2-0.2,0.3-0.5,0.1-0.7L6.4,171.5z"/>
	  <polyline points="-1.5,181.5 5.7,181.5 5.7,180.7 -1.5,180.7"/>
      </svg>;

const REFRESH_ICON = <svg version="1.1" viewBox="0 0 25 25">
   <g>
	<path d="M12.7,20.3c-2,0-4.1-0.8-5.6-2.3c-1.6-1.6-2.3-3.6-2.3-5.6c0-0.8,0.1-1.7,0.4-2.5l1.6,1.4L6.5,6.5 L1.8,7.1l1.8,1.5C3,9.8,2.8,11.1,2.8,12.4c0,2.5,1,5.1,2.9,7c1.9,1.9,4.5,2.9,7,2.9c1.6,0,3.3-0.4,4.8-1.2L16,19.6 C14.9,20.1,13.8,20.3,12.7,20.3z"/>
	<path d="M21.4,17c0.8-1.4,1.1-3,1.1-4.6c0-2.5-1-5.1-2.9-7c-1.9-1.9-4.5-2.9-7-2.9c-1.9,0-3.8,0.6-5.5,1.6 l1.4,1.4c1.2-0.7,2.6-1.1,4-1.1c2,0,4.1,0.8,5.6,2.3c1.6,1.6,2.3,3.6,2.3,5.6c0,1.1-0.3,2.3-0.7,3.3l-1.7-1.4l0.2,3.8L18.3,18 c0,0,0,0,0,0l0.1,0.1l0.1,0.9l4.7-0.6L21.4,17z"/>
  </g>
  </svg>;

class AddGrain extends React.Component {
  props: {};
  state: {};

  constructor(props) {
    super(props);
  }

  handleClick(event) {
    event.preventDefault();
    doRequest(interfaces.uiView);
  }

  render() {
    return <tr className="add-grain" onClick={this.handleClick}>
      <td/>
      <td className="install-icon">
       {INSTALL_ICON}
      </td>
      <td><button>Add grain...</button></td>
      <td/>
      </tr>;
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
           userId: String,
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

  selectGrain(token, e) {
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
    http("/offer/" + token, "post");
  }

  searchStringChange(e) {
    this.setState({ searchString: e.target.value});
  }

  matchesAppOrGrainTitle = function (needle, grain, info) {
    if (!info || info.err) return false;
    if (grain && grain.title && grain.title.toLowerCase().indexOf(needle) !== -1) return true;
    if (info.ok && info.ok.appTitle && info.ok.appTitle.toLowerCase().indexOf(needle) !== -1) {
      return true;
    }
    return false;
  }

  refresh(token) {
    http("/refresh/" + token, "post");
  }

  remove(token){
    http("/sturdyref/" + token, "delete");
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
    const grains = [];
    for (let e of this.props.grains.entries()) {
      const grain = e[1];
      const info = this.props.viewInfos.get(e[0]) || {};
      if ((info.ok || this.props.canWrite) && matchFilter(grain, info)) {
        if (this.state.selectedGrains.get(e[0])) {
          numShownAndSelected += 1;
        }
        this._currentlyRendered[e[0]] = true;

        grains.push({token: e[0], grain, info });
      }
    }
    const grainRows = _.chain(grains).sortBy((r) => r.grain.dateAdded).map((r) => {
      const checkbox = this.props.canWrite ?
           <td onClick={this.clickCheckboxContainer.bind(this)}>
            <input type="checkbox" checked={!!this.state.selectedGrains.get(r.token)}
                    onChange={this.selectGrain.bind(this, r.token)}/></td>
        : [];
      const appIcon = r.info.ok ?
            <td className="td-app-icon click-to-go" onClick={this.offerUiView.bind(this, r.token)}>
             <img title={r.info.ok.appTitle} src={r.info.ok.grainIconUrl} className="grain-icon">
             </img>
            </td> :
        <td className="td-app-icon">
        <button className="secondary-button" name="refresh" title="retry connection to grain"
                onClick={this.refresh.bind(this, r.token)}>{REFRESH_ICON}</button>
        </td> ;

      const grainTitle = r.info.ok ?
            <td className="click-to-go" onClick={this.offerUiView.bind(this, r.token)}>
            <button onClick={(e) => {e.preventDefault();} }>{r.grain.title}</button>
            </td> :
            <td><span className="broken-link" title={"broken: " + r.info.err}>
             {r.grain.title}</span>
             <span>(broken)
             </span>
           </td>;

      const dateAdded = r.info.ok?
          <td className="click-to-go date-added" onClick={this.offerUiView.bind(this, r.token)}>
              {makeDateString(new Date(parseInt(r.grain.dateAdded)))}
          </td> :
          <td className="date-added">{makeDateString(new Date(parseInt(r.grain.dateAdded)))}</td>;

      return <tr className={r.info.ok ? "grain" : "broken-grain"} key={r.token}>
          {checkbox}{appIcon}{grainTitle}{dateAdded}
        </tr>;
    }).value();

    const bulkActionButtons = [];
    if (this.props.canWrite) {
      bulkActionButtons.push(
          <button key="unlink"
                  disabled={numShownAndSelected==0}
                 title={numShownAndSelected==0 ?
                        "select grains to unlink them" : "unlink selected grains"}
                  onClick={this.clickRemoveGrain.bind(this)}>Unlink from collection</button>);
    }

    return <div className="grain-list">
      <div className="search-row">
      <label>
      {SEARCH_ICON}
      <input className="search-bar" type="text" placeholder="search"
             onChange={this.searchStringChange.bind(this)}/>
      </label>
      </div>
      <div className="bulk-action-buttons">
      {bulkActionButtons}
    </div>
      <table className="grain-list-table">
          <thead>
           <tr>
         {this.props.canWrite ?
            <td onClick={this.clickCheckboxContainer.bind(this)}
              className="select-all-grains">
          <input type="checkbox" title={numShownAndSelected > 0 ? "unselect all" : "select all"}
                 onChange={this.selectAll.bind(this)}
                 checked={numShownAndSelected > 0}/>
           </td> : [] }
              <td className="td-app-icon"></td>
              <td className="grain-name">Name</td>
              <td className="date-added">Date added</td>
      {/*<td className="added-by">Added by</td>*/}
            </tr>
          </thead>
      <tbody>
      {(this.props.canWrite && this.props.userId && !this.state.searchString) ? <AddGrain/>: [] }
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

  clickCancel(e) {
    e.preventDefault();
    this.setState({ editing: false, editedDescription: this.props.description });
  }

  submitEdit(e) {
    e.preventDefault();
    if (this.state.editedDescription !== this.props.description) {
      http("/description", "put", this.state.editedDescription);
    }
    this.setState({ editing: false });
  }

  changeDesc(e) {
    this.setState({ editedDescription: e.target.value });
  }

  render () {
    if (this.state.editing) {
      return <form className="description-row" onSubmit={this.submitEdit.bind(this)}>
        <input type="text" onChange={this.changeDesc.bind(this)}
               defaultValue={this.props.description} autoFocus={true}>
        </input>
        <button className="primary-button" title="done editing">done</button>
        <button className="secondary-button" title="cancel"
                onClick={this.clickCancel.bind(this)}>cancel</button>
       </form>;
    } else if (this.props.description && this.props.description.length > 0) {
      let button = [];
      if (this.props.canWrite) {
        button = <button className="description-button"
                         title="edit description"
                         onClick={this.clickEdit.bind(this)}>{EDIT_ICON}</button>;
      }
      return <div className="description-row"><p>{this.props.description}</p>
      {button}
      </div>;
    } else {
      if (this.props.canWrite) {
        return <button className="secondary-button" title="add description"
                       onClick={this.clickEdit.bind(this)}>Add description</button>
      } else {
        return null;
      }
    }
  }
}

class Main extends React.Component {
  props: {};
  state: { canWrite: bool,
           userId: String,
           description: String,
           grains: Immutable.Map,
           viewInfos: Immutable.Map,
           socketReadyState: Object,
         };

  constructor(props) {
    super(props);
    this.state = { grains: Immutable.Map(),
                   viewInfos: Immutable.Map(),
                   socketReadyState: { initializing: true},
                 };
  }

  componentDidMount() {
    this.openWebSocket(1000);
  }

  openWebSocket(delayOnFailure) {
    if (!!this.state.socketReadyState.open) {
      return;
    }

    this.setState({socketReadyState: { connecting: true } });

    let wsProtocol = window.location.protocol == "http:" ? "ws" : "wss";
    let ws = new WebSocket(wsProtocol + "://" + window.location.host);

    ws.onopen = (e) => {
      this.setState({ socketReadyState: { open: true } });
    };

    ws.onerror = (e) => {
      console.log("websocket got error: ", e);
    };

    ws.onclose = (e) => {
      console.log("websocket closed: ", e);
      let newDelay = 0;
      if (!this.state.socketReadyState.open) {
        if (delayOnFailure == 0) {
          newDelay = 1000;
        } else {
          newDelay = Math.min(delayOnFailure * 2, 60 * 1000); // Don't go over a minute.
        }
        console.log("websocket failed to connect. Retrying in " + delayOnFailure + " milliseconds");
      }

      const timeout = window.setTimeout(() => {
        this.openWebSocket(newDelay);
      }, delayOnFailure);

      this.setState({
        socketReadyState: {
          tryingAgainLater: { timeout }
        }
      });

    };

    ws.onmessage = (m) => {
      const action = JSON.parse(m.data);
      if (action.canWrite) {
        this.setState({canWrite: action.canWrite});
      } else if (action.userId) {
        this.setState({userId: action.userId});
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
        const data = action.viewInfo.data ?
              { ok: action.viewInfo.data } :
              { err: action.viewInfo.failed.split("\n")[0] }; // HACK to drop the stack trace.

        const newViewInfos = this.state.viewInfos.set(action.viewInfo.token, data);
        this.setState({ viewInfos: newViewInfos });
      }
    };

  }

  retryConnect() {
    if (this.state.socketReadyState.tryingAgainLater) {
      window.clearTimeout(this.state.socketReadyState.tryingAgainLater.timeout);
      this.openWebSocket(1000);
    }
  }

  render() {
    let maybeSocketWarning = null;
    if (!!this.state.socketReadyState.connecting) {
      maybeSocketWarning = <p>WebSocket connecting...</p>;
    } else if (!!this.state.socketReadyState.tryingAgainLater) {
      // TODO display timer for how long until next retry
      maybeSocketWarning = <p>WebSocket closed! Waiting and then retrying...
        <button className="secondary-button" onClick={this.retryConnect.bind(this)}>
         retry now
        </button>
        </p>;
    }

    return <div>
      {maybeSocketWarning}
      <Description canWrite={this.state.canWrite} description={this.state.description}/>
      <hr/>
      <GrainList grains={this.state.grains} viewInfos={this.state.viewInfos}
                 canWrite={this.state.canWrite} userId={this.state.userId} />
      </div>;
  }
}

ReactDOM.render(<Main/>,  document.getElementById("main"));


