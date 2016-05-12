require("babel-polyfill");
const React = require('react');
const ReactDOM = require('react-dom');

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
  window.parent.postMessage({
    powerboxRequest: {
      rpcId: "1",
      query: [serializedPowerboxDescriptor],
    },
  }, "*");
}


class AddGrain extends React.Component {
  props: {};
  state: {};

  constructor(props) {
    super(props);
  }

  handleClick(event) {
    console.log("clicked");
    doRequest(interfaces.uiView);
  }

  render() {
    return <button onClick={this.handleClick}> AddGrain </button>;
  }
}

ReactDOM.render(
  <div><h1>Collections</h1>
    <main>
      <AddGrain/>
    </main>
  </div>,
  document.getElementById("main")
);

window.addEventListener('message', (event) => {
  console.log("got postmessage: " + JSON.stringify(event.data));
});
