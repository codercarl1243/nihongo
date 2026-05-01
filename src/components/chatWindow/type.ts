 
    export type Sender = "user" | "bot";
    
    export type Message = {
      sender: Sender;
      text: string;
    };
    
    export type ChatState = {
      message: string;
      messageArray: Message[];
    };
    
    export type ChatAction =
      | { type: "SET_MESSAGE"; payload: string }
      | { type: "SEND_MESSAGE"; payload: string };
    