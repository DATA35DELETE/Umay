package com.example.thecommunication;

import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.recyclerview.widget.RecyclerView;
import java.util.List;

public class ContactAdapter extends RecyclerView.Adapter<ContactAdapter.ContactViewHolder> {

    private List<Contact> contactList;
    private OnItemClickListener listener;
    private OnItemLongClickListener longClickListener;

    public interface OnItemClickListener {
        void onItemClick(Contact contact);
    }

    public interface OnItemLongClickListener {
        boolean onItemLongClick(Contact contact);
    }

    public ContactAdapter(List<Contact> contactList, OnItemClickListener listener, OnItemLongClickListener longClickListener) {
        this.contactList = contactList;
        this.listener = listener;
        this.longClickListener = longClickListener;
    }

    @NonNull
    @Override
    public ContactViewHolder onCreateViewHolder(@NonNull ViewGroup parent, int viewType) {
        View view = LayoutInflater.from(parent.getContext())
                .inflate(R.layout.item_contact, parent, false);
        return new ContactViewHolder(view);
    }

    @Override
    public void onBindViewHolder(@NonNull ContactViewHolder holder, int position) {
        Contact contact = contactList.get(position);
        holder.bind(contact, listener, longClickListener);
    }

    @Override
    public int getItemCount() {
        return contactList.size();
    }

    static class ContactViewHolder extends RecyclerView.ViewHolder {
        TextView nameTextView;
        TextView messageTextView;
        TextView timeTextView;

        public ContactViewHolder(@NonNull View itemView) {
            super(itemView);
            nameTextView = itemView.findViewById(R.id.textViewName);
            messageTextView = itemView.findViewById(R.id.textViewLastMessage);
            timeTextView = itemView.findViewById(R.id.textViewTime);
        }

        public void bind(final Contact contact, final OnItemClickListener listener, final OnItemLongClickListener longClickListener) {
            nameTextView.setText(contact.getName());
            messageTextView.setText(contact.getLastMessage());
            timeTextView.setText(contact.getTime());

            // Normal click - open chat (bağlantı ChatActivity'de kurulacak)
            itemView.setOnClickListener(v -> {
                listener.onItemClick(contact);
            });

            // Long click - delete
            itemView.setOnLongClickListener(v -> {
                if (longClickListener != null) {
                    return longClickListener.onItemLongClick(contact);
                }
                return false;
            });
        }
    }
}