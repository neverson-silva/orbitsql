use tedious_rs::data_type::type_by_name;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::request::Request;
use tedious_rs::value_codec::Value;

#[test]
fn make_params_parameter_preserves_original_numeric_declarations() {
    let mut request = Request::new("SELECT @numeric, @decimal", |_| {});
    let options = ParameterOptions {
        precision: Some(12),
        scale: Some(4),
        ..ParameterOptions::default()
    };
    request
        .add_parameter(
            "numeric",
            type_by_name("Numeric").unwrap(),
            Value::Decimal {
                unscaled: 12345678,
                scale: 4,
            },
            options.clone(),
        )
        .unwrap();
    request
        .add_parameter(
            "decimal",
            type_by_name("Decimal").unwrap(),
            Value::Decimal {
                unscaled: 12345678,
                scale: 4,
            },
            options,
        )
        .unwrap();
    assert_eq!(
        request.make_params_parameter().unwrap(),
        "@numeric numeric(12, 4), @decimal decimal(12, 4)"
    );
    assert_eq!(
        request.make_params_declaration().unwrap(),
        "@numeric numeric(12, 4), @decimal decimal(12, 4)"
    );
}

#[test]
fn make_params_parameter_supports_intn_declaration() {
    let mut request = Request::new("SELECT @int_n", |_| {});
    request
        .add_parameter(
            "int_n",
            type_by_name("IntN").unwrap(),
            Value::Integer(1),
            ParameterOptions::default(),
        )
        .unwrap();
    assert_eq!(request.make_params_parameter().unwrap(), "@int_n int");
}

#[test]
fn make_params_parameter_supports_numericn_declarations() {
    for name in ["NumericN", "DecimalN"] {
        let mut request = Request::new("SELECT @value", |_| {});
        request
            .add_parameter(
                "value",
                type_by_name(name).unwrap(),
                Value::Decimal {
                    unscaled: 1234,
                    scale: 2,
                },
                ParameterOptions::default(),
            )
            .unwrap();

        let expected = if name == "NumericN" {
            "@value numeric(18, 0)"
        } else {
            "@value decimal(18, 0)"
        };
        assert_eq!(request.make_params_parameter().unwrap(), expected);
    }
}
