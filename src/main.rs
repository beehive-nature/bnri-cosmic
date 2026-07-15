// BNRi COSMIC — inscription explorer + bLOVErAi interface
// Built on libcosmic (Rust GUI toolkit by System76/COSMIC)
// SPDX-License-Identifier: AGPL-3.0-only
//
// Thin binary (G-0): the view layer only. The modules live in the library
// crate, so this file declares none of them.

use cosmic::iced::{self, Alignment, Length};
use cosmic::widget::{self, button, column, container, row, scrollable, text, text_input};
use cosmic::theme::Theme;
use cosmic::Application;

use bnri_cosmic::TransactionQuote;
use bnri_cosmic::{agent, hex_renderer, wallet};

// ──────────────────────────────────────────────────────────
// App state
// ──────────────────────────────────────────────────────────

pub struct BnriApp {
    core: cosmic::app::Core,
    state: AppState,
    // Kernel connection
    kernel: KernelHandle,
    // View state
    current_view: View,
    // BNRi inscription data
    inscriptions: Vec<InscriptionPreview>,
    // bLOVErAi chat
    chat_messages: Vec<ChatMessage>,
    chat_input: String,
    // Wallet
    wallet_state: WalletState,
}

pub enum AppState {
    Initializing,
    Connected,
    Offline,
}

pub enum View {
    Gallery,        // BNRi inscription gallery (art-first)
    Inscription(usize),  // Detail view for a specific inscription
    Chat,           // bLOVErAi interface
    Wallet,         // Wallet + transactions
    NodeStatus,     // Full-node status (Autonomi, b-indexer, community serving)
    Farming,        // Farming cycle tracker
}

pub struct InscriptionPreview {
    id: String,
    name: String,
    level: u8,
    level_name: String,
    hd_type: Option<String>,
    aura_color: String,
    locked: bool,
    sealed_tokens: Option<u64>,
    network: String,
    is_eternal: bool,
    // Cached hex-pixel bitmap (rendered from HexRect[] via canvas)
    bitmap: Option<image::DynamicImage>,
}

pub struct ChatMessage {
    sender: MessageSender,
    text: String,
    timestamp: u64,
}

pub enum MessageSender {
    Bloverai,
    Human,
    System,
}

pub struct WalletState {
    bnri_balance: u64,
    btc_balance: u64,
    b_balance: u64,  // kernel accounting (earned metabolic energy)
    bnr_balance: u64, // BNRi raw units
    locked_bnri: u64,
    pending_rewards: u64,
}

pub struct KernelHandle {
    // Connection to Beehive kernel (via Unix socket or IPC)
    connected: bool,
}

// ──────────────────────────────────────────────────────────
// COSMIC Application trait implementation
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    SwitchView(View),
    SelectInscription(usize),
    
    // Gallery
    RefreshInscriptions,
    InscriptionsLoaded(Vec<InscriptionPreview>),
    
    // bLOVErAi chat
    ChatInputChanged(String),
    SendMessage,
    MessageReceived(ChatMessage),
    
    // Wallet
    RefreshWallet,
    WalletUpdated(WalletState),
    
    // Transaction simulation (bLOVErAi quote flow)
    SimulateTransaction(String),  // action description
    TransactionQuote(TransactionQuote),
    ConfirmTransaction,
    CancelTransaction,
    
    // Node status
    RefreshNodeStatus,
    NodeStatusUpdated(NodeStatus),
    
    // Kernel connection
    KernelConnected,
    KernelDisconnected,
    
    // Art rendering
    BitmapReady(usize, image::DynamicImage),
    
    // Theme
    ToggleTheme,
}

pub struct NodeStatus {
    autonomi_connected: bool,
    autonomi_earnings_today: f64,
    b_indexer_synced: bool,
    community_serving: bool,
    llm_model: String,
    llm_ram_usage_gb: f64,
    vps_connected: bool,
}

impl Application for BnriApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(core: cosmic::app::Core, _flags: Self::Flags) -> (Self, cosmic::app::Task<Self::Message>) {
        let app = BnriApp {
            core,
            state: AppState::Initializing,
            kernel: KernelHandle { connected: false },
            current_view: View::Gallery,
            inscriptions: Vec::new(),
            chat_messages: vec![
                ChatMessage {
                    sender: MessageSender::Bloverai,
                    text: "Hello. I'm bLOVErAi. I'm here with you, locally. Nothing leaves this machine.".to_string(),
                    timestamp: 0,
                }
            ],
            chat_input: String::new(),
            wallet_state: WalletState {
                bnri_balance: 0,
                btc_balance: 0,
                b_balance: 0,
                bnr_balance: 0,
                locked_bnri: 0,
                pending_rewards: 0,
            },
        };
        
        // TODO: Initialize kernel connection via Unix socket
        // TODO: Load inscription cache from Autonomi/local
        // TODO: Start LLM sidecar (GLM-5.2) if RAM permits
        
        (app, cosmic::app::Task::none())
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::SwitchView(view) => {
                self.current_view = view;
            }
            
            Message::SelectInscription(idx) => {
                self.current_view = View::Inscription(idx);
            }
            
            Message::RefreshInscriptions => {
                // TODO: Fetch inscriptions from chain-exsat-evm adapter
                // For now, return empty
            }
            
            Message::InscriptionsLoaded(inscriptions) => {
                self.inscriptions = inscriptions;
            }
            
            Message::ChatInputChanged(text) => {
                self.chat_input = text;
            }
            
            Message::SendMessage => {
                if !self.chat_input.is_empty() {
                    let msg = ChatMessage {
                        sender: MessageSender::Human,
                        text: self.chat_input.clone(),
                        timestamp: chrono::Utc::now().timestamp() as u64,
                    };
                    self.chat_messages.push(msg);
                    self.chat_input.clear();
                    
                    // TODO: Send to bLOVErAi LLM sidecar (local GLM-5.2)
                    // The sidecar processes locally and responds
                    // bLOVErAi never sends data to any external service
                }
            }
            
            Message::MessageReceived(msg) => {
                self.chat_messages.push(msg);
            }
            
            Message::RefreshWallet => {
                // TODO: Query exSat EVM for BNRi balance
                // TODO: Query kernel for b-token balance (internal accounting)
            }
            
            Message::WalletUpdated(state) => {
                self.wallet_state = state;
            }
            
            Message::SimulateTransaction(action) => {
                // TODO: bLOVErAi simulates via eth_estimateGas + eth_call
                // Returns quote in b-token (kernel accounting)
                // This is the CONSENT-1 disclose-and-confirm pattern
            }
            
            Message::TransactionQuote(quote) => {
                // Display quote to user
                // User must confirm — bLOVErAi never signs
                // Human signs with Trezor (large tx) or device keystore (small tx)
            }
            
            Message::ConfirmTransaction => {
                // TODO: Route to paymaster with kernel-signed voucher
                // Paymaster sponsors gas (BTC), kernel debits b
            }
            
            Message::CancelTransaction => {
                // Nothing happens. No gas spent. No griefing possible.
            }
            
            Message::RefreshNodeStatus => {
                // TODO: Query kernel resource manager
            }
            
            Message::NodeStatusUpdated(status) => {
                // Update node status display
            }
            
            Message::KernelConnected => {
                self.state = AppState::Connected;
                self.kernel.connected = true;
            }
            
            Message::KernelDisconnected => {
                self.state = AppState::Offline;
                self.kernel.connected = false;
            }
            
            Message::BitmapReady(idx, bitmap) => {
                if idx < self.inscriptions.len() {
                    self.inscriptions[idx].bitmap = Some(bitmap);
                }
            }
            
            Message::ToggleTheme => {
                // Toggle between light/dark (default: dark — 80s/90s cypherpunk-raver)
            }
        }
        
        cosmic::app::Task::none()
    }

    fn view(&self) -> cosmic::app::Element<Self::Message> {
        // ── Navigation sidebar ──────────────────────────────
        let nav = column![
            button::text("GALLERY").on_press(Message::SwitchView(View::Gallery)),
            button::text("bLOVErAi").on_press(Message::SwitchView(View::Chat)),
            button::text("WALLET").on_press(Message::SwitchView(View::Wallet)),
            button::text("FARMING").on_press(Message::SwitchView(View::Farming)),
            button::text("NODE").on_press(Message::SwitchView(View::NodeStatus)),
        ]
        .spacing(8)
        .padding(16)
        .align_items(Alignment::Start);

        // ── Main content area (switches based on view) ─────
        let content: cosmic::app::Element<Self::Message> = match self.current_view {
            View::Gallery => self.view_gallery(),
            View::Inscription(idx) => self.view_inscription(idx),
            View::Chat => self.view_chat(),
            View::Wallet => self.view_wallet(),
            View::NodeStatus => self.view_node_status(),
            View::Farming => self.view_farming(),
        };

        // ── Layout: sidebar | content ───────────────────────
        let layout = row![nav, content]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

// ──────────────────────────────────────────────────────────
// View implementations (stubs — wired to kernel, not yet rendering)
// ──────────────────────────────────────────────────────────

impl BnriApp {
    fn view_gallery(&self) -> cosmic::app::Element<Self::Message> {
        // Art-first gallery: hex-pixel bee cards in a grid
        // Each card shows: art (dominant) + 4 facts (value, lock, network, eternal tag)
        // Click → detail view
        // 
        // TODO: Implement hex-pixel canvas renderer (hex_renderer module)
        // TODO: Fetch inscription list from chain-exsat-evm adapter
        // TODO: Cache rendered bitmaps (never inline SVG in DOM — frontend law)
        
        column![
            text("BNRi INSCRIPTION GALLERY").size(24),
            text(format!("{} inscriptions", self.inscriptions.len())),
            text("Art-first. Complexity behind reveals.").size(12),
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
    
    fn view_inscription(&self, idx: usize) -> cosmic::app::Element<Self::Message> {
        // Detail view: art large + expandable reveals
        // + IDENTITY / + ACCESSORIES / + RENDER DATA / + OWNERSHIP
        // Seam 1 sentence visible in ownership panel for eternal
        //
        // TODO: Fetch full ItemData from getItemData(itemId)
        // TODO: Render hex pixels via canvas (hex_renderer module)
        
        column![
            text(format!("Inscription #{}", idx)).size(24),
            text("[ HEX PIXEL ART — canvas render ]").size(16),
            text("+ IDENTITY").size(14),
            text("+ ACCESSORIES").size(14),
            text("+ RENDER DATA").size(14),
            text("+ OWNERSHIP").size(14),
            button::text("← BACK").on_press(Message::SwitchView(View::Gallery)),
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
    
    fn view_chat(&self) -> cosmic::app::Element<Self::Message> {
        // bLOVErAi chat interface
        // Private — never leaves the machine
        // bLOVErAi can simulate transactions, show b-token quotes
        // She never signs — the human signs after the quote
        //
        // TODO: Wire to LLM sidecar (GLM-5.2 via Unix socket)
        // TODO: Transaction quote overlay (CONSENT-1 pattern)
        
        let messages: Vec<cosmic::app::Element<Self::Message>> = self.chat_messages
            .iter()
            .map(|msg| {
                let prefix = match msg.sender {
                    MessageSender::Bloverai => "bLOVErAi",
                    MessageSender::Human => "You",
                    MessageSender::System => "System",
                };
                text(format!("{}: {}", prefix, msg.text)).size(14).into()
            })
            .collect();
        
        let chat_scroll = scrollable(column(messages))
            .width(Length::Fill)
            .height(Length::FillPortion(4));
        
        let input_row = row![
            text_input("Talk to bLOVErAi...", &self.chat_input)
                .on_input(Message::ChatInputChanged)
                .on_submit(Message::SendMessage)
                .width(Length::FillPortion(4)),
            button::text("SEND").on_press(Message::SendMessage),
        ]
        .spacing(8)
        .padding(8);
        
        column![chat_scroll, input_row]
            .spacing(0)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    
    fn view_wallet(&self) -> cosmic::app::Element<Self::Message> {
        // Wallet: art-first, complexity hidden
        // Shows: BNRi balance, b-token balance, locked inscriptions
        // Hides: gas price, BTC balance (unless expanded), seed data
        // Large txs → "Press Trezor button"
        // Small txs → biometric/device keystore
        //
        // TODO: Wire to exSat EVM RPC for BNRi balance
        // TODO: Wire to kernel for b-token balance (internal accounting)
        // TODO: Trezor integration via kernel's dro-signer crate
        
        column![
            text("WALLET").size(24),
            text(format!("BNRi: {:.2}", self.wallet_state.bnri_balance as f64 / 100.0)).size(18),
            text(format!("b (metabolic): {:.4}", self.wallet_state.b_balance as f64 / 1e18)).size(14),
            text(format!("Locked BNRi: {:.2}", self.wallet_state.locked_bnri as f64 / 100.0)).size(14),
            text(format!("Pending rewards: {:.4} b", self.wallet_state.pending_rewards as f64 / 1e18)).size(12),
            button::text("REFRESH").on_press(Message::RefreshWallet),
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
    
    fn view_node_status(&self) -> cosmic::app::Element<Self::Message> {
        // Full-node status: Autonomi, b-indexer, community serving, LLM
        // Shows earnings (ANT + b-token)
        // Shows adaptive resource state (model selection, RAM usage)
        //
        // TODO: Wire to kernel resource manager
        
        column![
            text("NODE STATUS").size(24),
            text("Autonomi: [checking...]").size(14),
            text("b-indexer: [checking...]").size(14),
            text("LLM: GLM-5.2 [checking...]").size(14),
            text("Community serving: [checking...]").size(14),
            text("Earnings today: [checking...]").size(14),
            button::text("REFRESH").on_press(Message::RefreshNodeStatus),
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
    
    fn view_farming(&self) -> cosmic::app::Element<Self::Message> {
        // Farming cycle tracker: BQueen Bee / Caffeine / e
        // Shows: current cycle, ticket count, snapshot/draw dates
        // bQueenBee's ticket count publicly labeled (verifiable odds)
        //
        // TODO: Wire to BNRiFarming contract events via chain-exsat-evm
        
        column![
            text("FARMING CYCLES").size(24),
            text("Cycle 1: BQueen Bee Genesis (days 0-90)").size(16),
            text("Cycle 2: Caffeine (days 180-270)").size(16),
            text("Cycle 3: e (days 360-450)").size(16),
            text("Status: [checking...]").size(14),
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
}

// ──────────────────────────────────────────────────────────
// Main entry point
// ──────────────────────────────────────────────────────────

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt::init();
    
    cosmic::app::run::<BnriApp>(
        cosmic::app::Settings::default()
            .size((1280, 800))
            .theme(|_app: &BnriApp| Theme::Dark),  // Default dark — 80s/90s cypherpunk-raver
    )
}
