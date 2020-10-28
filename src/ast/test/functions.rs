use super::*;

yes! {
    function_literal => r#"
    test! <- 3.
    "#
}

no! {
    function_colon_dash => r#"
    test! :- 3.
    "#
}

yes! {
    function_arguments => r#"
    test!(A, B) <- A + B.
    "#
}

yes! {
    function_assumptions => r#"
    test!(A, B) <-
        C <- A + 2,
        D <- B - 2,
        C * D.
    "#
}

#[cfg(feature = "builtin-sets")]
yes! {
    function_aggregate_set => r#"
    node(_, _, _).
    test!(A, B) <- { pair(X, Y) : node(A, X, _), node(B, _, Y) }.
    "#
}

yes! {
    function_aggregate_list => r#"
    node(_, _, _).
    test!(A, B) <- [ pair(X, Y) : node(A, X, _), node(B, _, Y) ].
    "#
}

no! {
    function_unifications => r#"
    test(a).
    test!(A, B) <-
        test(A),
        test(B),
        A + B.
    "#
}

yes! {
    function_return_struct => r#"
    test!(A, B) <- test(A, B).
    "#
}

yes! {
    function_call => r#"
    test!(A) <- A + 2.
    test!(A, B) <- test!(A) + test!(B).
    "#
}

// TODO: this will probably be a future feature
no! {
    function_call_nested => r#"
    call!(A) <- A + 2.
    test!(A) <- call!(call!(A)).
    "#
}

no! {
    function_call_undefined => r#"
    test!(A) <- call!(A).
    "#
}

yes! {
    function_lists => r#"
    len!([]) <- 0.
    len!([_, ..R]) <- 1 + len!(R).
    "#
}
