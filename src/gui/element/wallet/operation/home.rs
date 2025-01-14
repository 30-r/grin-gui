use super::tx_list::{self, ExpandType};
use async_std::prelude::FutureExt;
use grin_gui_core::{
    config::Config,
    wallet::{TxLogEntry, TxLogEntryType, RetrieveTxQueryArgs},
};
use iced_aw::Card;
use iced_native::Widget;
use std::path::PathBuf;

use super::{action_menu, tx_list_display};
use super::tx_list::{HeaderState, TxList, TxLogEntryWrap};
use grin_gui_widgets::widget::header;


use {
    super::super::super::{
        DEFAULT_FONT_SIZE, DEFAULT_HEADER_FONT_SIZE, DEFAULT_PADDING, DEFAULT_SUB_HEADER_FONT_SIZE,
        SMALLER_FONT_SIZE,
    },
    crate::gui::{GrinGui, Interaction, Message},
    crate::localization::localized_string,
    crate::log_error,
    crate::Result,
    anyhow::Context,
    grin_gui_core::theme::{
        Button, Column, Container, Element, PickList, Row, Scrollable, Text, TextInput, Header, TableRow
    },
    grin_gui_core::wallet::{StatusMessage, WalletInfo, WalletInterface},
    grin_gui_core::{node::amount_to_hr_string, theme::ColorPalette},
    iced::widget::{button, pick_list, scrollable, text_input, Checkbox, Space},
    iced::{alignment, Alignment, Command, Length},
    std::sync::{Arc, RwLock},
};

pub struct StateContainer {
    pub action_menu_state: action_menu::StateContainer,
    pub tx_list_display_state: tx_list_display::StateContainer,

    wallet_info: Option<WalletInfo>,
    wallet_txs: TxList,
    wallet_status: String,
    // txs_scrollable_state: scrollable::State,
    last_summary_update: chrono::DateTime<chrono::Local>,
    tx_header_state: HeaderState,
}

impl Default for StateContainer {
    fn default() -> Self {
        Self {
            action_menu_state: Default::default(),
            tx_list_display_state: Default::default(),
            // back_button_state: Default::default(),
            wallet_info: Default::default(),
            wallet_txs: Default::default(),
            wallet_status: Default::default(),
            // txs_scrollable_state: Default::default(),
            last_summary_update: Default::default(),
            tx_header_state: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LocalViewInteraction {
    Back,
    Submit,
    /// was updated from node, info
    WalletInfoUpdateSuccess(bool, WalletInfo, Vec<TxLogEntry>),
    WalletInfoUpdateFailure(Arc<RwLock<Option<anyhow::Error>>>),
    WalletSlatepackAddressUpdateSuccess(String),
    WalletCloseError(Arc<RwLock<Option<anyhow::Error>>>),
    WalletCloseSuccess,
    CancelTx(u32),
    TxDetails(TxLogEntryWrap),
    TxCancelledOk(u32),
    TxCancelError(Arc<RwLock<Option<anyhow::Error>>>)
}

// Okay to modify state and access wallet here
pub fn handle_tick<'a>(
    grin_gui: &mut GrinGui,
    time: chrono::DateTime<chrono::Local>,
) -> Result<Command<Message>> {
    {
        let w = grin_gui.wallet_interface.read().unwrap();
        if !w.wallet_is_open() {
            return Ok(Command::none());
        }
    }
    let messages = WalletInterface::get_wallet_updater_status(grin_gui.wallet_interface.clone())?;
    let state = &mut grin_gui.wallet_state.operation_state.home_state;
    let last_message = messages.get(0);
    if let Some(m) = last_message {
        state.wallet_status = match m {
            StatusMessage::UpdatingOutputs(s) => format!("{}", s),
            StatusMessage::UpdatingTransactions(s) => format!("{}", s),
            StatusMessage::FullScanWarn(s) => format!("{}", s),
            StatusMessage::Scanning(s, m) => format!("Scanning - {}% complete", m),
            StatusMessage::ScanningComplete(s) => format!("{}", s),
            StatusMessage::UpdateWarning(s) => format!("{}", s),
        }
    }
    if time - state.last_summary_update
        > chrono::Duration::from_std(std::time::Duration::from_secs(10)).unwrap()
    {
        state.last_summary_update = chrono::Local::now();

        let mut query_args = RetrieveTxQueryArgs::default();

        query_args.exclude_cancelled = Some(true);
        query_args.include_outstanding_only = Some(true);

        let w = grin_gui.wallet_interface.clone();

        let fut =
            move || WalletInterface::get_wallet_info(w.clone());//.join(WalletInterface::get_txs(w, Some(query_args)));

        //return Ok(Command::perform(fut(), |(wallet_info_res, txs_res)| {
        return Ok(Command::perform(fut(), |(wallet_info_res)| {
            if wallet_info_res.is_err() {
                let e = wallet_info_res
                    .context("Failed to retrieve wallet info status")
                    .unwrap_err();
                return Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                    LocalViewInteraction::WalletInfoUpdateFailure(Arc::new(RwLock::new(Some(e)))),
                ));
            }
            /*if txs_res.is_err() {
                let e = txs_res
                    .context("Failed to retrieve wallet tx status")
                    .unwrap_err();
                return Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                    LocalViewInteraction::WalletInfoUpdateFailure(Arc::new(RwLock::new(Some(e)))),
                ));
            }*/
            let (node_success, wallet_info) = wallet_info_res.unwrap();
            //let (_, txs) = txs_res.unwrap();
            Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                //LocalViewInteraction::WalletInfoUpdateSuccess(node_success, wallet_info, txs),
                LocalViewInteraction::WalletInfoUpdateSuccess(node_success, wallet_info, vec![]),
            ))
        }));
    }
    // If slatepack address is not filled out, go get it
    let apply_tx_state = &mut grin_gui.wallet_state.operation_state.apply_tx_state;
    if apply_tx_state.address_value.is_empty() {
        let w = grin_gui.wallet_interface.clone();

        let fut = move || WalletInterface::get_slatepack_address(w.clone());
        return Ok(Command::perform(fut(), |get_slatepack_address_res| {
            if get_slatepack_address_res.is_err() {
                let e = get_slatepack_address_res
                    .context("Failed to retrieve wallet slatepack address")
                    .unwrap_err();
                return Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                    LocalViewInteraction::WalletInfoUpdateFailure(Arc::new(RwLock::new(Some(e)))),
                ));
            }
            Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                LocalViewInteraction::WalletSlatepackAddressUpdateSuccess(
                    get_slatepack_address_res.unwrap(),
                ),
            ))
        }));
    }
    Ok(Command::none())
}

pub fn handle_message<'a>(
    grin_gui: &mut GrinGui,
    message: LocalViewInteraction,
) -> Result<Command<Message>> {
    let state = &mut grin_gui.wallet_state.operation_state.home_state;
    match message {
        LocalViewInteraction::Back => {
            let wallet_interface = grin_gui.wallet_interface.clone();
            let fut = WalletInterface::close_wallet(wallet_interface);

            return Ok(Command::perform(fut, |r| {
                match r.context("Failed to close wallet") {
                    Ok(()) => {
                        Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                            LocalViewInteraction::WalletCloseSuccess,
                        ))
                    }
                    Err(e) => {
                        Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                            LocalViewInteraction::WalletCloseError(Arc::new(RwLock::new(Some(e)))),
                        ))
                    }
                }
            }));
        }
        LocalViewInteraction::Submit => {}
        LocalViewInteraction::WalletInfoUpdateSuccess(node_success, wallet_info, txs) => {
            debug!(
                "Update Wallet Info Summary: {}, {:?}",
                node_success, wallet_info
            );
            state.wallet_info = Some(wallet_info);
            debug!("Update Wallet Txs Summary: {:?}", txs);
            let tx_wrap_list = txs
                .iter()
                .map(|tx| TxLogEntryWrap::new(tx.clone()))
                .collect();
            state.wallet_txs = TxList { txs: tx_wrap_list };
        }
        LocalViewInteraction::WalletInfoUpdateFailure(err) => {
            grin_gui.error = err.write().unwrap().take();
            if let Some(e) = grin_gui.error.as_ref() {
                log_error(e);
            }
        }
        LocalViewInteraction::WalletCloseSuccess => {
            grin_gui.wallet_state.operation_state.mode =
                crate::gui::element::wallet::operation::Mode::Open;
        }
        LocalViewInteraction::WalletCloseError(err) => {
            grin_gui.error = err.write().unwrap().take();
            if let Some(e) = grin_gui.error.as_ref() {
                log_error(e);
            }
        }
        LocalViewInteraction::WalletSlatepackAddressUpdateSuccess(address) => {
            grin_gui
                .wallet_state
                .operation_state
                .apply_tx_state
                .address_value = address;
        }
        LocalViewInteraction::TxDetails(tx_log_entry_wrap) => {
            log::debug!("Interaction::WalletOperationHomeViewInteraction::TxDetails");
            log::debug!("TBD {}", tx_log_entry_wrap.tx.id);
        }
        LocalViewInteraction::CancelTx(id) => {
            debug!("Cancel Tx: {}", id);
            grin_gui.error.take();

            log::debug!("Interaction::WalletOperationHomeViewInteraction::CancelTx");

            let w = grin_gui.wallet_interface.clone();

            let fut = move || WalletInterface::cancel_tx(w, id);

            return Ok(Command::perform(fut(), |r| {
                match r.context("Failed to Cancel Transaction") {
                    Ok(ret) => Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                        LocalViewInteraction::TxCancelledOk(ret),
                    )),
                    Err(e) => Message::Interaction(Interaction::WalletOperationHomeViewInteraction(
                        LocalViewInteraction::TxCancelError(Arc::new(RwLock::new(Some(e)))),
                    )),
                }
            }));
        },
        LocalViewInteraction::TxCancelledOk(id) => {
            //TODO: Message
            debug!("TX cancelled okay: {}", id);
        },
        LocalViewInteraction::TxCancelError(err) => {
            grin_gui.error = err.write().unwrap().take();
            if let Some(e) = grin_gui.error.as_ref() {
                log_error(e);
            }
        }

        
    }
    Ok(Command::none())
}

pub fn data_container<'a>(
    config: &'a Config,
    state: &'a StateContainer,
) -> Container<'a, Message> {
    // Buttons to perform operations go here, but empty container for now
    let operations_menu =
        action_menu::data_container(config, &state.action_menu_state);

    // Basic Info "Box"
    let waiting_string = "---------";
    let (
        total_string,
        amount_spendable_string,
        awaiting_confirmation_string,
        awaiting_finalization_string,
        locked_string,
    ) = match state.wallet_info.as_ref() {
        Some(info) => (
            amount_to_hr_string(info.total, false),
            amount_to_hr_string(info.amount_currently_spendable, false),
            amount_to_hr_string(info.amount_awaiting_confirmation, false),
            amount_to_hr_string(info.amount_awaiting_finalization, false),
            amount_to_hr_string(info.amount_locked, false),
        ),
        None => (
            waiting_string.to_owned(),
            waiting_string.to_owned(),
            waiting_string.to_owned(),
            waiting_string.to_owned(),
            waiting_string.to_owned(),
        ),
    };

    let wallet_name = config.wallets[config.current_wallet_index.unwrap()]
        .display_name
        .clone();

    // Title row
    let title = Text::new(amount_spendable_string.clone()).size(DEFAULT_HEADER_FONT_SIZE);
    let title_container = Container::new(title)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground);

    let subtitle = Text::new(wallet_name).size(SMALLER_FONT_SIZE);
    let subtitle_container = Container::new(subtitle)
        .style(grin_gui_core::theme::ContainerStyle::NormalBackground)
        .padding(iced::Padding::from([
            3, // top
            0, // right
            0, // bottom
            0, // left
        ]));

    let close_wallet_label_container =
        Container::new(Text::new(localized_string("close")).size(SMALLER_FONT_SIZE))
            .height(Length::Units(14))
            .width(Length::Units(30))
            .center_y()
            .center_x();

    let close_wallet_button: Element<Interaction> = Button::new(close_wallet_label_container)
        .style(grin_gui_core::theme::ButtonStyle::Bordered)
        .on_press(Interaction::WalletOperationHomeViewInteraction(
            LocalViewInteraction::Back,
        ))
        .padding(2)
        .into();

    let subtitle_row = Row::new()
        .push(subtitle_container)
        .push(Space::with_width(Length::Units(2)))
        .push(close_wallet_button.map(Message::Interaction));

    let title_container = Container::new(Column::new().push(title_container).push(subtitle_row)).padding(iced::Padding::from([
        0,               // top
        0,               // right
        0,               // bottom
        5,               // left
    ]));

    let header_row = Row::new()
        .push(title_container)
        .push(Space::with_width(Length::Fill))
        .push(operations_menu);

    let header_container = Container::new(header_row).padding(iced::Padding::from([
        0,               // top
        0,               // right
        DEFAULT_PADDING, // bottom
        0,               // left
    ]));

    let total_value_label =
        Text::new(format!("{}:", localized_string("info-confirmed-total"))).size(DEFAULT_FONT_SIZE);
    let total_value_label_container = Container::new(total_value_label)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground);

    let total_value = Text::new(total_string).size(DEFAULT_FONT_SIZE);
    let total_value_container = Container::new(total_value)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Right);

    let total_row = Row::new()
        .push(total_value_label_container)
        .push(total_value_container)
        .width(Length::Fill);

    let awaiting_confirmation_label = Text::new(format!(
        "{}:",
        localized_string("info-awaiting-confirmation")
    ))
    .size(DEFAULT_FONT_SIZE);
    let awaiting_confirmation_label_container = Container::new(awaiting_confirmation_label)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground);

    let awaiting_confirmation_value =
        Text::new(awaiting_confirmation_string).size(DEFAULT_FONT_SIZE);
    let awaiting_confirmation_value_container = Container::new(awaiting_confirmation_value)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Right);

    let awaiting_confirmation_row = Row::new()
        .push(awaiting_confirmation_label_container)
        .push(awaiting_confirmation_value_container)
        .width(Length::Fill);

    let awaiting_finalization_label = Text::new(format!(
        "{}:",
        localized_string("info-awaiting-finalization")
    ))
    .size(DEFAULT_FONT_SIZE);
    let awaiting_finalization_label_container = Container::new(awaiting_finalization_label)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground);

    let awaiting_finalization_value =
        Text::new(awaiting_finalization_string).size(DEFAULT_FONT_SIZE);
    let awaiting_finalization_value_container = Container::new(awaiting_finalization_value)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Right);

    let awaiting_finalization_row = Row::new()
        .push(awaiting_finalization_label_container)
        .push(awaiting_finalization_value_container)
        .width(Length::Fill);

    let locked_label =
        Text::new(format!("{}:", localized_string("info-locked"))).size(DEFAULT_FONT_SIZE);
    let locked_label_container = Container::new(locked_label)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground);

    let locked_value = Text::new(locked_string).size(DEFAULT_FONT_SIZE);
    let locked_value_container = Container::new(locked_value)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Right);

    let locked_row = Row::new()
        .push(locked_label_container)
        .push(locked_value_container)
        .width(Length::Fill);

    let amount_spendable_label =
        Text::new(format!("{}:", localized_string("info-amount-spendable")))
            .size(DEFAULT_FONT_SIZE);
    let amount_spendable_label_container = Container::new(amount_spendable_label)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground);

    let amount_spendable_value = Text::new(amount_spendable_string).size(DEFAULT_FONT_SIZE);
    let amount_spendable_value_container = Container::new(amount_spendable_value)
        .style(grin_gui_core::theme::ContainerStyle::BrightBackground)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Right);

    let amount_spendable_row = Row::new()
        .push(amount_spendable_label_container)
        .push(amount_spendable_value_container)
        .width(Length::Fill);

    let info_column = Column::new()
        .push(total_row)
        .push(awaiting_confirmation_row)
        .push(awaiting_finalization_row)
        .push(locked_row)
        .push(amount_spendable_row)
        .spacing(7);

    let wallet_info_card_container = Container::new(info_column)
        .width(Length::Units(240))
        .padding(iced::Padding::from([
            DEFAULT_PADDING, // top
            DEFAULT_PADDING, // right
            DEFAULT_PADDING, // bottom
            5,               // left
        ]));

    let first_row_container = Row::new().push(wallet_info_card_container);

    // Status container bar at bottom of screen
    let status_container_label_text = Text::new(localized_string("status"))
        .size(DEFAULT_FONT_SIZE)
        .height(Length::Fill)
        .horizontal_alignment(alignment::Horizontal::Right)
        .vertical_alignment(alignment::Vertical::Center);

    let status_container_separator_text = Text::new(": ")
        .size(DEFAULT_FONT_SIZE)
        .height(Length::Fill)
        .horizontal_alignment(alignment::Horizontal::Right)
        .vertical_alignment(alignment::Vertical::Center);

    let status_container_status_text = Text::new(&state.wallet_status)
        .size(DEFAULT_FONT_SIZE)
        .height(Length::Fill)
        .horizontal_alignment(alignment::Horizontal::Right)
        .vertical_alignment(alignment::Vertical::Center);

    let status_container_contents = Row::new()
        .push(Space::new(Length::Fill, Length::Fill))
        .push(status_container_label_text)
        .push(status_container_separator_text)
        .push(status_container_status_text)
        .push(Space::new(Length::Units(DEFAULT_PADDING), Length::Units(0)));

    let status_container = Container::new(status_container_contents)
        .style(grin_gui_core::theme::ContainerStyle::BrightForeground)
        .height(Length::Fill)
        .width(Length::Fill)
        .style(grin_gui_core::theme::ContainerStyle::NormalBackground);

    let status_row = Row::new()
        .push(status_container)
        //.height(Length::Units(25))
        .align_items(Alignment::Center)
        .spacing(25);

    // Buttons to perform operations go here, but empty container for now
    let tx_list_display =
        tx_list_display::data_container(config, &state.tx_list_display_state);

   // Overall Home screen layout column
    let column = Column::new()
        .push(header_container)
        .push(first_row_container)
        .push(Space::new(Length::Units(0), Length::Units(10)))
        .push(tx_list_display)
        .push(status_row)
        .padding(10);
        //.align_items(Alignment::Center);

    Container::new(column).padding(iced::Padding::from([
        DEFAULT_PADDING, // top
        DEFAULT_PADDING, // right
        DEFAULT_PADDING, // bottom
        DEFAULT_PADDING, // left
    ]))
}
