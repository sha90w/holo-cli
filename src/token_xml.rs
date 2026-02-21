//
// Copyright (c) The Holo Core Contributors
//
// SPDX-License-Identifier: MIT
//

use indextree::NodeId;
use xml::ParserConfig;
use xml::reader::XmlEvent;

use crate::internal_commands;
use crate::token::{Action, Commands, Token, TokenKind};

pub fn gen_cmds(commands: &mut Commands) {
    // Read embedded XML file containing command definitions.
    let xml = include_str!("internal_commands.xml");
    let reader = ParserConfig::new().create_reader(xml.as_bytes());

    // Iterate over all XML tags.
    let mut stack = vec![];
    for e in reader {
        match e {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                let token_id = match name.local_name.as_str() {
                    "tree" => parse_tag_tree(commands, attributes),
                    "token" => {
                        let parent = stack.last().unwrap();
                        parse_tag_token(commands, *parent, attributes)
                    }
                    // Ignore unknown tags for now.
                    _ => continue,
                };

                // Update stack of tokens.
                stack.push(token_id);
            }
            Ok(XmlEvent::EndElement { .. }) => {
                // Update stack of tokens.
                stack.pop();
            }
            Ok(_) => (),
            Err(e) => panic!("Error parsing XML document: {:?}", e),
        }
    }
}

fn parse_tag_tree(
    commands: &Commands,
    attributes: Vec<xml::attribute::OwnedAttribute>,
) -> NodeId {
    let name = find_attribute(&attributes, "name");
    match name {
        "exec" => commands.exec_root,
        "config" => commands.config_root_internal,
        "config-default" => commands.config_dflt_internal,
        "pipe" => commands.pipe_root,
        _ => panic!("unknown tree name: {}", name),
    }
}

fn parse_tag_token(
    commands: &mut Commands,
    parent: NodeId,
    attributes: Vec<xml::attribute::OwnedAttribute>,
) -> NodeId {
    let name = find_attribute(&attributes, "name");
    let help = find_opt_attribute(&attributes, "help");
    let kind = find_opt_attribute(&attributes, "kind");
    let argument = find_opt_attribute(&attributes, "argument");
    let cmd_name = find_opt_attribute(&attributes, "cmd");
    let pipeable = find_opt_attribute(&attributes, "pipeable") == Some("true");

    let action = cmd_name.map(|name| match name {
        "cmd_config" => Action::Callback(internal_commands::cmd_config),
        "cmd_list" => Action::Callback(internal_commands::cmd_list),
        "cmd_exit_exec" => Action::Callback(internal_commands::cmd_exit_exec),
        "cmd_exit_config" => {
            Action::Callback(internal_commands::cmd_exit_config)
        }
        "cmd_end" => Action::Callback(internal_commands::cmd_end),
        "cmd_pwd" => Action::Callback(internal_commands::cmd_pwd),
        "cmd_top" => Action::Callback(internal_commands::cmd_top),
        "cmd_discard" => Action::Callback(internal_commands::cmd_discard),
        "cmd_commit" => Action::Callback(internal_commands::cmd_commit),
        "cmd_validate" => Action::Callback(internal_commands::cmd_validate),
        "cmd_show_config" => {
            Action::Callback(internal_commands::cmd_show_config)
        }
        "cmd_show_config_changes" => {
            Action::Callback(internal_commands::cmd_show_config_changes)
        }
        "cmd_show_state" => Action::Callback(internal_commands::cmd_show_state),
        "cmd_show_yang_modules" => {
            Action::Callback(internal_commands::cmd_show_yang_modules)
        }
        "cmd_show_isis_interface" => {
            Action::Callback(internal_commands::cmd_show_isis_interface)
        }
        "cmd_show_isis_adjacency" => {
            Action::Callback(internal_commands::cmd_show_isis_adjacency)
        }
        "cmd_show_isis_database" => {
            Action::Callback(internal_commands::cmd_show_isis_database)
        }
        "cmd_show_isis_route" => {
            Action::Callback(internal_commands::cmd_show_isis_route)
        }
        "cmd_show_ospf_interface" => {
            Action::Callback(internal_commands::cmd_show_ospf_interface)
        }
        "cmd_show_ospf_interface_detail" => {
            Action::Callback(internal_commands::cmd_show_ospf_interface_detail)
        }
        "cmd_show_ospf_vlink" => {
            Action::Callback(internal_commands::cmd_show_ospf_vlink)
        }
        "cmd_show_ospf_neighbor" => {
            Action::Callback(internal_commands::cmd_show_ospf_neighbor)
        }
        "cmd_show_ospf_neighbor_detail" => {
            Action::Callback(internal_commands::cmd_show_ospf_neighbor_detail)
        }
        "cmd_show_ospf_database_as" => {
            Action::Callback(internal_commands::cmd_show_ospf_database_as)
        }
        "cmd_show_ospf_database_area" => {
            Action::Callback(internal_commands::cmd_show_ospf_database_area)
        }
        "cmd_show_ospf_database_link" => {
            Action::Callback(internal_commands::cmd_show_ospf_database_link)
        }
        "cmd_show_ospf_route" => {
            Action::Callback(internal_commands::cmd_show_ospf_route)
        }
        "cmd_show_ospf_hostnames" => {
            Action::Callback(internal_commands::cmd_show_ospf_hostnames)
        }
        "cmd_show_rip_interface" => {
            Action::Callback(internal_commands::cmd_show_rip_interface)
        }
        "cmd_show_rip_interface_detail" => {
            Action::Callback(internal_commands::cmd_show_rip_interface_detail)
        }
        "cmd_show_rip_neighbor" => {
            Action::Callback(internal_commands::cmd_show_rip_neighbor)
        }
        "cmd_show_rip_neighbor_detail" => {
            Action::Callback(internal_commands::cmd_show_rip_neighbor_detail)
        }
        "cmd_show_rip_route" => {
            Action::Callback(internal_commands::cmd_show_rip_route)
        }
        "cmd_show_mpls_ldp_discovery" => {
            Action::Callback(internal_commands::cmd_show_mpls_ldp_discovery)
        }
        "cmd_show_mpls_ldp_discovery_detail" => Action::Callback(
            internal_commands::cmd_show_mpls_ldp_discovery_detail,
        ),
        "cmd_show_mpls_ldp_peer" => {
            Action::Callback(internal_commands::cmd_show_mpls_ldp_peer)
        }
        "cmd_show_mpls_ldp_peer_detail" => {
            Action::Callback(internal_commands::cmd_show_mpls_ldp_peer_detail)
        }
        "cmd_show_mpls_ldp_binding_address" => Action::Callback(
            internal_commands::cmd_show_mpls_ldp_binding_address,
        ),
        "cmd_show_mpls_ldp_binding_fec" => {
            Action::Callback(internal_commands::cmd_show_mpls_ldp_binding_fec)
        }
        "cmd_clear_isis_adjacency" => {
            Action::Callback(internal_commands::cmd_clear_isis_adjacency)
        }
        "cmd_clear_isis_database" => {
            Action::Callback(internal_commands::cmd_clear_isis_database)
        }
        "pipe_include" => Action::PipeCallback(internal_commands::pipe_include),
        "pipe_exclude" => Action::PipeCallback(internal_commands::pipe_exclude),
        _ => panic!("unknown command name: {}", name),
    });

    let kind = match kind {
        Some("string") => TokenKind::String,
        Some(_) => panic!("unknown token kind"),
        None => TokenKind::Word,
    };

    // Add new token.
    let mut token = Token::new(name, help, kind, argument, action, false);
    token.pipeable = pipeable;

    // Link new token.
    commands.add_token(parent, token)
}

fn find_attribute<'a>(
    attributes: &'a [xml::attribute::OwnedAttribute],
    name: &str,
) -> &'a str {
    find_opt_attribute(attributes, name).unwrap_or_else(|| {
        panic!("Failed to find mandatory {} XML attribute", name)
    })
}

fn find_opt_attribute<'a>(
    attributes: &'a [xml::attribute::OwnedAttribute],
    name: &str,
) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attr| attr.name.local_name == name)
        .map(|attr| attr.value.as_str())
}
