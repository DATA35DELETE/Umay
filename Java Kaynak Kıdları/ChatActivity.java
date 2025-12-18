package com.example.thecommunication;

import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.widget.EditText;
import android.widget.ImageButton;
import androidx.appcompat.app.AppCompatActivity;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import java.util.ArrayList;
import java.util.List;

public class ChatActivity extends AppCompatActivity {

    private static String activeContactId = null;

    private NativeLib nativeLib;
    private RecyclerView recyclerView;
    private MessageAdapter adapter;
    private List<Message> messageList;
    private EditText inputEdit;
    private Handler handler;

    private String contactName;
    private String contactId;
    private String contactAddress;
    private boolean connectionAttempted = false;

    public static boolean isActiveForPeer(String peerId) {
        return activeContactId != null && activeContactId.equals(peerId);
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_chat);

        nativeLib = NativeLib.getInstance();
        handler = new Handler(Looper.getMainLooper());

        contactName = getIntent().getStringExtra("CONTACT_NAME");
        contactId = getIntent().getStringExtra("CONTACT_ID");
        contactAddress = getIntent().getStringExtra("CONTACT_ADDRESS");

        android.util.Log.d("ChatActivity", "Opening chat with: " + contactName);
        android.util.Log.d("ChatActivity", "Contact ID: " + contactId);
        android.util.Log.d("ChatActivity", "Contact Address: " + contactAddress);

        // Set title
        if (getSupportActionBar() != null) {
            getSupportActionBar().setTitle(contactName);
            getSupportActionBar().setDisplayHomeAsUpEnabled(true);
        }

        recyclerView = findViewById(R.id.recyclerViewMessages);
        LinearLayoutManager layoutManager = new LinearLayoutManager(this);
        recyclerView.setLayoutManager(layoutManager);
        messageList = new ArrayList<>();
        adapter = new MessageAdapter(messageList);
        recyclerView.setAdapter(adapter);

        inputEdit = findViewById(R.id.editTextMessage);
        ImageButton sendButton = findViewById(R.id.buttonSend);

        sendButton.setOnClickListener(v -> sendMessage());

        // Enter tuşu ile mesaj gönder
        inputEdit.setOnEditorActionListener((v, actionId, event) -> {
            sendMessage();
            return true;
        });

        // İlk bağlantıyı kur
        attemptConnection();
    }

    private void attemptConnection() {
        if (contactAddress != null && !contactAddress.isEmpty() && !connectionAttempted) {
            connectionAttempted = true;
            android.util.Log.d("ChatActivity", "Initial connection to: " + contactAddress);
            nativeLib.dialPeer(contactAddress);

            // 3 saniye sonra tekrar dene (relay bağlantısı için)
            handler.postDelayed(() -> {
                android.util.Log.d("ChatActivity", "Retry connection...");
                nativeLib.dialPeer(contactAddress);
            }, 3000);

            // 6 saniye sonra bir kez daha
            handler.postDelayed(() -> {
                android.util.Log.d("ChatActivity", "Final retry connection...");
                nativeLib.dialPeer(contactAddress);
            }, 6000);
        }
    }

    private void sendMessage() {
        String text = inputEdit.getText().toString().trim();
        if (text.isEmpty()) {
            return;
        }

        // Eğer bağlantı daha yapılmadıysa, tekrar dene
        if (!connectionAttempted && contactAddress != null && !contactAddress.isEmpty()) {
            attemptConnection();
        }

        try {
            android.util.Log.d("ChatActivity", "Sending message to: " + contactId);
            nativeLib.sendMessage(contactId, text);

            // UI'a ekle
            String time = getCurrentTime();
            messageList.add(new Message(text, time, true));
            adapter.notifyItemInserted(messageList.size() - 1);
            recyclerView.scrollToPosition(messageList.size() - 1);
            inputEdit.setText("");

            // MainActivity'deki kişi listesini güncelle
            MainActivity.updateContactLastMessage(contactId, text, time);

        } catch (Exception e) {
            android.util.Log.e("ChatActivity", "Failed to send message", e);
            android.widget.Toast.makeText(this,
                    "Failed to send message. Retrying connection...",
                    android.widget.Toast.LENGTH_SHORT).show();

            // Bağlantıyı tekrar dene
            if (contactAddress != null && !contactAddress.isEmpty()) {
                nativeLib.dialPeer(contactAddress);
            }
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        activeContactId = contactId;

        android.util.Log.d("ChatActivity", "onResume - Setting listener for: " + contactId);

        // Bu konuşma için listener kur
        NativeLib.setListener((senderId, message) -> {
            android.util.Log.d("ChatActivity", "Message received from: " + senderId);
            android.util.Log.d("ChatActivity", "Expected contact: " + contactId);
            android.util.Log.d("ChatActivity", "Message: " + message);

            // Bu konuşmadan mı kontrol et
            if (contactId != null && contactId.equals(senderId)) {
                runOnUiThread(() -> {
                    android.util.Log.d("ChatActivity", "Adding message to UI");
                    String time = getCurrentTime();
                    messageList.add(new Message(message, time, false));
                    adapter.notifyItemInserted(messageList.size() - 1);
                    recyclerView.scrollToPosition(messageList.size() - 1);

                    // MainActivity'deki kişi listesini güncelle
                    MainActivity.updateContactLastMessage(contactId, message, time);
                });
            } else {
                android.util.Log.d("ChatActivity", "Message from different sender, ignoring");
            }
        });

        // Bağlantıyı kontrol et ve gerekirse yenile
        if (contactAddress != null && !contactAddress.isEmpty()) {
            handler.postDelayed(() -> {
                android.util.Log.d("ChatActivity", "onResume - Refreshing connection");
                nativeLib.dialPeer(contactAddress);
            }, 500);
        }
    }

    @Override
    protected void onPause() {
        super.onPause();
        activeContactId = null;
        android.util.Log.d("ChatActivity", "onPause - Clearing active contact");
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        activeContactId = null;

        // Handler cleanup
        if (handler != null) {
            handler.removeCallbacksAndMessages(null);
        }

        android.util.Log.d("ChatActivity", "onDestroy");
    }

    @Override
    public boolean onSupportNavigateUp() {
        finish();
        return true;
    }

    private String getCurrentTime() {
        return new java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault())
                .format(new java.util.Date());
    }
}