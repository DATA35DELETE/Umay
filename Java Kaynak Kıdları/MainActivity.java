package com.example.thecommunication;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.widget.ImageButton;
import android.widget.TextView;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.appcompat.app.AppCompatActivity;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import com.google.android.material.floatingactionbutton.FloatingActionButton;
import com.google.gson.Gson;
import com.google.gson.reflect.TypeToken;
import java.io.File;
import java.lang.reflect.Type;
import java.util.ArrayList;
import java.util.List;

public class MainActivity extends AppCompatActivity {

    private static boolean nodeInitialized = false;
    private static MainActivity instance;

    private NativeLib nativeLib;
    private RecyclerView recyclerView;
    private ContactAdapter adapter;
    private List<Contact> contactList;

    private TextView textViewPeerId;
    private TextView textViewRelayAddress;
    private ImageButton buttonCopyPeerId;
    private ImageButton buttonCopyRelayAddress;
    private ImageButton buttonQRCode;
    private Handler handler;

    private final ActivityResultLauncher<Intent> qrResultLauncher = registerForActivityResult(
            new ActivityResultContracts.StartActivityForResult(),
            result -> {
                if (result.getResultCode() == Activity.RESULT_OK && result.getData() != null) {
                    String scannedPeerId = result.getData().getStringExtra("SCANNED_PEER_ID");
                    String scannedAddress = result.getData().getStringExtra("SCANNED_ADDRESS");
                    handleScannedContact(scannedPeerId, scannedAddress);
                }
            }
    );

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        instance = this;

        android.util.Log.d("MainActivity", "========== APP STARTING ==========");
        android.util.Log.d("MainActivity", "Android API Level: " + android.os.Build.VERSION.SDK_INT);

        setContentView(R.layout.activity_main);

        // Create notification channel
        NotificationHelper.createNotificationChannel(this);

        String error = NativeLib.init();
        android.util.Log.d("MainActivity", "Native lib init: " + (error == null ? "SUCCESS" : error));

        if (error != null) {
            android.util.Log.e("MainActivity", "Failed to load native lib: " + error);
            android.widget.Toast.makeText(this, "Native library error: " + error,
                    android.widget.Toast.LENGTH_LONG).show();
            new androidx.appcompat.app.AlertDialog.Builder(this)
                    .setTitle("Library Error")
                    .setMessage("Failed to load native library: " + error)
                    .setPositiveButton("OK", (d, w) -> finish())
                    .show();
            return;
        }

        try {
            nativeLib = NativeLib.getInstance();
            android.util.Log.d("MainActivity", "NativeLib instance created");

            File identityFile = new File(getFilesDir(), "identity.key");
            String identityPath = identityFile.getAbsolutePath();
            android.util.Log.i("MainActivity", "Identity path: " + identityPath);

            if (!nodeInitialized) {
                android.util.Log.d("MainActivity", "Starting P2P node...");
                nativeLib.startNode("", identityPath);
                nodeInitialized = true;
                android.util.Log.d("MainActivity", "P2P node started");

                new Handler(Looper.getMainLooper()).postDelayed(() -> {
                    android.util.Log.d("MainActivity", "Checking peer ID after delay...");
                    checkPeerIdNow();
                }, 3000);
            } else {
                android.util.Log.d("MainActivity", "P2P node already running");
            }

            textViewPeerId = findViewById(R.id.textViewPeerId);
            textViewRelayAddress = findViewById(R.id.textViewRelayAddress);
            buttonCopyPeerId = findViewById(R.id.buttonCopyPeerId);
            buttonCopyRelayAddress = findViewById(R.id.buttonCopyRelayAddress);
            buttonQRCode = findViewById(R.id.buttonQRCode);

            handler = new Handler(Looper.getMainLooper());

            buttonCopyPeerId.setOnClickListener(v -> {
                String peerId = textViewPeerId.getText().toString();
                if (!peerId.equals("Loading...") && !peerId.equals("Unknown")) {
                    copyToClipboard("Peer ID", peerId);
                    android.widget.Toast.makeText(this, "Peer ID copied!", android.widget.Toast.LENGTH_SHORT).show();
                } else {
                    android.widget.Toast.makeText(this, "Peer ID not ready yet", android.widget.Toast.LENGTH_SHORT).show();
                }
            });

            buttonCopyRelayAddress.setOnClickListener(v -> {
                String relayAddr = textViewRelayAddress.getText().toString();
                if (!relayAddr.equals("Connecting...") && !relayAddr.equals("No relay address yet")) {
                    copyToClipboard("Relay Address", relayAddr);
                    android.widget.Toast.makeText(this, "Relay Address copied!", android.widget.Toast.LENGTH_SHORT).show();
                } else {
                    android.widget.Toast.makeText(this, "Relay address not ready yet", android.widget.Toast.LENGTH_SHORT).show();
                }
            });

            buttonQRCode.setOnClickListener(v -> {
                Intent intent = new Intent(MainActivity.this, QRActivity.class);
                intent.putExtra("MY_PEER_ID", textViewPeerId.getText().toString());
                intent.putExtra("MY_RELAY_ADDRESS", textViewRelayAddress.getText().toString());
                qrResultLauncher.launch(intent);
            });

            updatePeerInfo();

            recyclerView = findViewById(R.id.recyclerViewContacts);
            recyclerView.setLayoutManager(new LinearLayoutManager(this));

            loadContacts();

            adapter = new ContactAdapter(contactList,
                    contact -> {
                        Intent intent = new Intent(MainActivity.this, ChatActivity.class);
                        intent.putExtra("CONTACT_NAME", contact.getName());
                        intent.putExtra("CONTACT_ID", contact.getPeerId());
                        intent.putExtra("CONTACT_ADDRESS", contact.getAddress());
                        startActivity(intent);
                    },
                    contact -> {
                        showDeleteContactDialog(contact);
                        return true;
                    });
            recyclerView.setAdapter(adapter);

            FloatingActionButton fab = findViewById(R.id.fabNewChat);
            fab.setOnClickListener(v -> showAddContactDialog());

            setupGlobalMessageListener();

            android.util.Log.d("MainActivity", "========== APP STARTED SUCCESSFULLY ==========");

        } catch (Exception e) {
            android.util.Log.e("MainActivity", "========== FATAL ERROR ==========", e);
            android.widget.Toast.makeText(this, "Initialization error: " + e.getMessage(),
                    android.widget.Toast.LENGTH_LONG).show();
        }
    }

    private void handleScannedContact(String peerId, String address) {
        if (peerId == null || peerId.isEmpty()) {
            android.widget.Toast.makeText(this, "Invalid QR code", android.widget.Toast.LENGTH_SHORT).show();
            return;
        }

        // Check if contact already exists
        Contact existingContact = findContactByPeerId(peerId);
        if (existingContact != null) {
            android.widget.Toast.makeText(this,
                    "Contact already exists: " + existingContact.getName(),
                    android.widget.Toast.LENGTH_SHORT).show();
            return;
        }

        // Show dialog to name the contact
        android.widget.EditText nameInput = new android.widget.EditText(this);
        nameInput.setHint("Contact Name");
        nameInput.setText("User-" + peerId.substring(Math.max(0, peerId.length() - 8)));

        android.widget.LinearLayout layout = new android.widget.LinearLayout(this);
        layout.setOrientation(android.widget.LinearLayout.VERTICAL);
        layout.setPadding(32, 32, 32, 32);
        layout.addView(nameInput);

        new androidx.appcompat.app.AlertDialog.Builder(this)
                .setTitle("Add Scanned Contact")
                .setMessage("Peer ID: " + peerId)
                .setView(layout)
                .setPositiveButton("Add", (dialog, which) -> {
                    String name = nameInput.getText().toString().trim();
                    if (name.isEmpty()) {
                        name = "User-" + peerId.substring(Math.max(0, peerId.length() - 8));
                    }

                    if (!address.isEmpty()) {
                        android.util.Log.d("MainActivity", "Dialing scanned peer: " + address);
                        nativeLib.dialPeer(address);
                        nativeLib.saveContact(name, address);
                    }

                    Contact newContact = new Contact(name, peerId, address, "Tap to chat", "Now");
                    contactList.add(0, newContact);
                    adapter.notifyItemInserted(0);
                    recyclerView.scrollToPosition(0);
                    saveContacts();

                    android.widget.Toast.makeText(this,
                            "Contact added: " + name,
                            android.widget.Toast.LENGTH_SHORT).show();
                })
                .setNegativeButton("Cancel", null)
                .show();
    }

    private void setupGlobalMessageListener() {
        NativeLib.setListener((senderId, message) -> {
            android.util.Log.d("MainActivity", "Global listener - Message from: " + senderId + " - " + message);

            runOnUiThread(() -> {
                // ChatActivity açıksa ve bu mesaj o konuşmadan geliyorsa, işleme
                if (ChatActivity.isActiveForPeer(senderId)) {
                    android.util.Log.d("MainActivity", "ChatActivity active for this peer, skipping");
                    return;
                }

                Contact existingContact = findContactByPeerId(senderId);

                if (existingContact == null) {
                    android.util.Log.d("MainActivity", "New contact detected, adding: " + senderId);

                    String contactName = "User-" + senderId.substring(Math.max(0, senderId.length() - 8));
                    Contact newContact = new Contact(
                            contactName,
                            senderId,
                            "",
                            message.length() > 30 ? message.substring(0, 30) + "..." : message,
                            getCurrentTime()
                    );

                    contactList.add(0, newContact);
                    adapter.notifyItemInserted(0);
                    recyclerView.scrollToPosition(0);
                    saveContacts();

                    // Show notification
                    NotificationHelper.showMessageNotification(this, contactName, message, senderId, "");

                    android.widget.Toast.makeText(this,
                            "New message from " + contactName,
                            android.widget.Toast.LENGTH_SHORT).show();
                } else {
                    android.util.Log.d("MainActivity", "Updating existing contact: " + existingContact.getName());

                    existingContact.setLastMessage(
                            message.length() > 30 ? message.substring(0, 30) + "..." : message,
                            getCurrentTime()
                    );

                    int position = contactList.indexOf(existingContact);
                    if (position != -1) {
                        contactList.remove(position);
                        contactList.add(0, existingContact);
                        adapter.notifyItemMoved(position, 0);
                        adapter.notifyItemChanged(0);
                    }
                    saveContacts();

                    // Show notification
                    NotificationHelper.showMessageNotification(this, existingContact.getName(), message, senderId, existingContact.getAddress());

                    android.widget.Toast.makeText(this,
                            "New message from " + existingContact.getName(),
                            android.widget.Toast.LENGTH_SHORT).show();
                }
            });
        });
    }

    public static void updateContactLastMessage(String peerId, String message, String time) {
        if (instance != null) {
            instance.runOnUiThread(() -> {
                Contact contact = instance.findContactByPeerId(peerId);
                if (contact != null) {
                    contact.setLastMessage(
                            message.length() > 30 ? message.substring(0, 30) + "..." : message,
                            time
                    );
                    int position = instance.contactList.indexOf(contact);
                    if (position != -1) {
                        instance.adapter.notifyItemChanged(position);
                    }
                    instance.saveContacts();
                }
            });
        }
    }

    private Contact findContactByPeerId(String peerId) {
        if (peerId == null || peerId.isEmpty()) return null;

        for (Contact contact : contactList) {
            if (peerId.equals(contact.getPeerId())) {
                return contact;
            }
        }
        return null;
    }

    private String getCurrentTime() {
        return new java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault())
                .format(new java.util.Date());
    }

    @Override
    protected void onResume() {
        super.onResume();
        instance = this;
        setupGlobalMessageListener();
        android.util.Log.d("MainActivity", "onResume - Global listener set");
    }

    @Override
    protected void onPause() {
        super.onPause();
        android.util.Log.d("MainActivity", "onPause");
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        if (handler != null) {
            handler.removeCallbacksAndMessages(null);
        }
        if (instance == this) {
            instance = null;
        }
    }

    private void checkPeerIdNow() {
        try {
            String peerId = nativeLib.getMyPeerId();
            android.util.Log.d("MainActivity", "Peer ID check: " + peerId);

            if (peerId != null && !peerId.equals("Unknown") && !peerId.isEmpty()) {
                textViewPeerId.setText(peerId);
                android.util.Log.d("MainActivity", "Peer ID set successfully: " + peerId);
            } else {
                android.util.Log.w("MainActivity", "Peer ID not ready yet");
            }
        } catch (Exception e) {
            android.util.Log.e("MainActivity", "Error checking peer ID", e);
        }
    }

    private void updatePeerInfo() {
        handler.postDelayed(new Runnable() {
            @Override
            public void run() {
                try {
                    String peerId = nativeLib.getMyPeerId();

                    if (peerId != null && !peerId.equals("Unknown") && !peerId.isEmpty()) {
                        String currentText = textViewPeerId.getText().toString();
                        if (!currentText.equals(peerId)) {
                            textViewPeerId.setText(peerId);
                        }
                    }

                    String[] addresses = nativeLib.getListenAddresses();
                    if (addresses != null && addresses.length > 0) {
                        String relayAddr = "No relay address yet";
                        for (String addr : addresses) {
                            if (addr.contains("p2p-circuit")) {
                                relayAddr = addr;
                                break;
                            }
                        }

                        String currentRelayText = textViewRelayAddress.getText().toString();
                        if (!currentRelayText.equals(relayAddr)) {
                            textViewRelayAddress.setText(relayAddr);
                        }
                    }
                } catch (Exception e) {
                    android.util.Log.e("MainActivity", "Error in updatePeerInfo", e);
                }

                handler.postDelayed(this, 2000);
            }
        }, 1000);
    }

    private void copyToClipboard(String label, String text) {
        ClipboardManager clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard != null) {
            ClipData clip = ClipData.newPlainText(label, text);
            clipboard.setPrimaryClip(clip);
        }
    }

    private void showDeleteContactDialog(Contact contact) {
        new androidx.appcompat.app.AlertDialog.Builder(this)
                .setTitle("Delete Contact")
                .setMessage("Are you sure you want to delete '" + contact.getName() + "'?")
                .setPositiveButton("Delete", (dialog, which) -> {
                    int position = contactList.indexOf(contact);
                    if (position != -1) {
                        contactList.remove(position);
                        adapter.notifyItemRemoved(position);
                        saveContacts();

                        android.widget.Toast.makeText(this,
                                contact.getName() + " deleted",
                                android.widget.Toast.LENGTH_SHORT).show();
                    }
                })
                .setNegativeButton("Cancel", null)
                .show();
    }

    private void showAddContactDialog() {
        android.widget.EditText nameInput = new android.widget.EditText(this);
        nameInput.setHint("Contact Name");

        android.widget.EditText idInput = new android.widget.EditText(this);
        idInput.setHint("Peer ID (e.g., 12D3K...)");

        android.widget.EditText addressInput = new android.widget.EditText(this);
        addressInput.setHint("Relay Address (optional)");

        android.widget.LinearLayout layout = new android.widget.LinearLayout(this);
        layout.setOrientation(android.widget.LinearLayout.VERTICAL);
        layout.setPadding(32, 32, 32, 32);
        layout.addView(nameInput);
        layout.addView(idInput);
        layout.addView(addressInput);

        new androidx.appcompat.app.AlertDialog.Builder(this)
                .setTitle("Add Contact")
                .setView(layout)
                .setPositiveButton("Add", (dialog, which) -> {
                    String name = nameInput.getText().toString().trim();
                    String peerId = idInput.getText().toString().trim();
                    String address = addressInput.getText().toString().trim();

                    if (name.isEmpty() || peerId.isEmpty()) {
                        android.widget.Toast.makeText(this,
                                "Name and Peer ID are required",
                                android.widget.Toast.LENGTH_SHORT).show();
                        return;
                    }

                    if (findContactByPeerId(peerId) != null) {
                        android.widget.Toast.makeText(this,
                                "Contact already exists",
                                android.widget.Toast.LENGTH_SHORT).show();
                        return;
                    }

                    if (!address.isEmpty()) {
                        android.util.Log.d("MainActivity", "Dialing peer: " + address);
                        nativeLib.dialPeer(address);
                        nativeLib.saveContact(name, address);
                    }

                    Contact newContact = new Contact(name, peerId, address, "Tap to chat", "Now");
                    contactList.add(0, newContact);
                    adapter.notifyItemInserted(0);
                    recyclerView.scrollToPosition(0);
                    saveContacts();

                    android.widget.Toast.makeText(this,
                            "Contact added!",
                            android.widget.Toast.LENGTH_SHORT).show();
                })
                .setNegativeButton("Cancel", null)
                .show();
    }

    private void saveContacts() {
        try {
            SharedPreferences prefs = getSharedPreferences("AppPrefs", MODE_PRIVATE);
            SharedPreferences.Editor editor = prefs.edit();
            Gson gson = new Gson();
            String json = gson.toJson(contactList);
            editor.putString("contacts", json);
            editor.apply();
        } catch (Exception e) {
            android.util.Log.e("MainActivity", "Error saving contacts", e);
        }
    }

    private void loadContacts() {
        try {
            SharedPreferences prefs = getSharedPreferences("AppPrefs", MODE_PRIVATE);
            String json = prefs.getString("contacts", null);
            contactList = new ArrayList<>();
            if (json != null) {
                Gson gson = new Gson();
                Type type = new TypeToken<ArrayList<Contact>>() {}.getType();
                contactList = gson.fromJson(json, type);
            }
        } catch (Exception e) {
            android.util.Log.e("MainActivity", "Error loading contacts", e);
            contactList = new ArrayList<>();
        }
    }
}