//
// Copyright (c) The Holo Core Contributors
//
// SPDX-License-Identifier: MIT
//

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use yang4::context::{Context, ContextFlags};

    use crate::error::ParserError;
    use crate::parser::{self, CommandPrefix};
    use crate::session::{CommandMode, Session};
    use crate::token::{Action, Commands, Token, TokenKind};
    use crate::YANG_CTX;

    /// Initialize YANG_CTX once for all tests (no modules needed).
    fn init_yang_ctx() {
        let _ = YANG_CTX.set(Arc::new(
            Context::new(ContextFlags::NO_YANGLIBRARY)
                .expect("Failed to create YANG context"),
        ));
    }

    /// Build a minimal command tree that mimics the real one:
    ///
    /// config-default children: set, delete, edit, up, exit, show,
    ///   commit
    ///
    /// Fake YANG tree under config_root_yang:
    ///   routing (container) → bgp (container) → as-number (leaf)
    ///
    /// set/edit/delete have subtree_root pointing at config_root_yang.
    fn build_test_commands() -> Commands {
        let mut cmds = Commands::new();

        // --- fake YANG tree ---
        fn leaf_token(name: &str) -> Token {
            Token::new(
                name,
                Some("help"),
                TokenKind::Word,
                Some(name),
                Some(Action::Callback(cmd_stub)),
                false,
                false,
            )
        }
        fn container_token(name: &str) -> Token {
            Token::new(
                name,
                Some("help"),
                TokenKind::Word,
                None,
                None,
                false,
                false,
            )
        }

        let routing_id =
            cmds.add_token(cmds.config_root_yang, container_token("routing"));
        let bgp_id = cmds.add_token(routing_id, container_token("bgp"));
        let _as_num_id = cmds.add_token(bgp_id, leaf_token("as-number"));

        // --- config-default XML commands ---
        fn cmd_token(name: &str) -> Token {
            Token::new(
                name,
                Some("help"),
                TokenKind::Word,
                None,
                Some(Action::Callback(cmd_stub)),
                false,
                false,
            )
        }

        let yang_root = cmds.config_root_yang;

        let mut set_tok = cmd_token("set");
        set_tok.subtree_root = Some(yang_root);
        cmds.add_token(cmds.config_dflt_internal, set_tok);

        let mut delete_tok = cmd_token("delete");
        delete_tok.subtree_root = Some(yang_root);
        cmds.add_token(cmds.config_dflt_internal, delete_tok);

        let mut edit_tok = cmd_token("edit");
        edit_tok.subtree_root = Some(yang_root);
        cmds.add_token(cmds.config_dflt_internal, edit_tok);

        cmds.add_token(cmds.config_dflt_internal, cmd_token("up"));
        cmds.add_token(cmds.config_dflt_internal, cmd_token("exit"));
        cmds.add_token(cmds.config_dflt_internal, cmd_token("commit"));

        // --- exec tree ---
        cmds.add_token(cmds.exec_root, cmd_token("configure"));
        cmds.add_token(cmds.exec_root, cmd_token("exit"));

        cmds
    }

    fn cmd_stub(
        _commands: &Commands,
        _session: &mut Session,
        _args: parser::ParsedArgs,
    ) -> Result<bool, crate::error::CallbackError> {
        Ok(false)
    }

    fn session_operational() -> Session {
        init_yang_ctx();
        Session::new_test(CommandMode::Operational)
    }

    fn session_configure() -> Session {
        init_yang_ctx();
        Session::new_test(CommandMode::Configure { nodes: vec![] })
    }

    // ===== Parser tests =====

    #[test]
    fn parse_operational_command() {
        let session = session_operational();
        let cmds = build_test_commands();
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.exec_root,
            "configure",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn parse_set_delegates_to_yang_tree() {
        let session = session_configure();
        let cmds = build_test_commands();
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "set routing bgp as-number",
        );
        assert!(result.is_ok());
        let pcmd = result.unwrap();
        assert_eq!(pcmd.prefix, CommandPrefix::Set);
        assert!(!pcmd.negate);
        // Should have collected the "as-number" argument.
        assert_eq!(pcmd.args.len(), 1);
        assert_eq!(pcmd.args[0].0, "as-number");
    }

    #[test]
    fn parse_delete_sets_negate() {
        let session = session_configure();
        let cmds = build_test_commands();
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "delete routing bgp as-number",
        );
        assert!(result.is_ok());
        let pcmd = result.unwrap();
        assert_eq!(pcmd.prefix, CommandPrefix::Delete);
        assert!(pcmd.negate);
    }

    #[test]
    fn parse_edit_sets_prefix() {
        let session = session_configure();
        let cmds = build_test_commands();
        // "edit routing" should parse into the YANG tree and stop
        // at "routing" container (which has no action → Incomplete).
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "edit routing",
        );
        match result {
            Err(ParserError::Incomplete(_, prefix)) => {
                assert_eq!(prefix, CommandPrefix::Edit);
            }
            other => panic!("expected Incomplete, got {:?}", other),
        }
    }

    #[test]
    fn parse_set_without_path_is_incomplete() {
        let session = session_configure();
        let cmds = build_test_commands();
        // "set" alone should be incomplete (at YANG root, no action).
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "set",
        );
        // After matching "set", curr_token_id moves to yang root
        // which has no action. But curr_token_id != start_token_id,
        // so it should be Incomplete.
        match result {
            Err(ParserError::Incomplete(..)) => {}
            other => panic!("expected Incomplete, got {:?}", other),
        }
    }

    #[test]
    fn parse_unknown_yang_path_fails() {
        let session = session_configure();
        let cmds = build_test_commands();
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "set nonexistent",
        );
        assert!(matches!(result, Err(ParserError::NoMatch(_))));
    }

    #[test]
    fn parse_builtin_command_in_config_mode() {
        let session = session_configure();
        let cmds = build_test_commands();
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "commit",
        );
        assert!(result.is_ok());
        let pcmd = result.unwrap();
        assert_eq!(pcmd.prefix, CommandPrefix::None);
    }

    #[test]
    fn config_mode_top_level_has_no_yang_tokens() {
        let session = session_configure();
        let cmds = build_test_commands();
        // Typing a bare YANG container name should fail (not at
        // top level).
        let result = parser::parse_command_try(
            &session,
            &cmds,
            cmds.config_dflt_internal,
            "routing",
        );
        assert!(matches!(result, Err(ParserError::NoMatch(_))));
    }

    // ===== Prompt tests =====

    #[test]
    fn prompt_operational_mode() {
        let mut session = session_operational();
        session.update_hostname();
        assert_eq!(session.prompt(), "holo");
    }

    #[test]
    fn prompt_configure_root() {
        let mut session = session_configure();
        session.update_hostname();
        let prompt = session.prompt();
        assert!(prompt.contains("[edit]"), "got: {}", prompt);
        assert!(prompt.contains("holo"), "got: {}", prompt);
        assert!(prompt.contains('\n'), "should be two-line: {}", prompt);
    }
}
