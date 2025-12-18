package com.example.thecommunication;

public class Message {
    private String content;
    private String time;
    private boolean isSentByMe;

    public Message(String content, String time, boolean isSentByMe) {
        this.content = content;
        this.time = time;
        this.isSentByMe = isSentByMe;
    }

    public String getContent() { return content; }
    public String getTime() { return time; }
    public boolean isSentByMe() { return isSentByMe; }
}