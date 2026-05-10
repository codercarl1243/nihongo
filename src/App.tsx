import "./styles/global.css";
import ChatWindow from "./components/chatWindow";
import ToastContainer from "./components/toast";

function App() {

  return (
    <main className="layout-wrapper ">
      <ChatWindow />
      <ToastContainer />
    </main>
  );
}

export default App;
