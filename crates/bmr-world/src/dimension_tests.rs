use super::*;

#[test]
fn vanilla_ids_parse_with_or_without_namespace() {
    assert_eq!(Dimension::parse("minecraft:overworld"), Dimension::Overworld);
    assert_eq!(Dimension::parse("minecraft:the_nether"), Dimension::Nether);
    assert_eq!(Dimension::parse("the_end"), Dimension::End);
    assert_eq!(Dimension::End.id(), "minecraft:the_end");
}

#[test]
fn unknown_ids_are_kept_as_other() {
    let d = Dimension::parse("mymod:mining");
    assert_eq!(d, Dimension::Other("mymod:mining".to_owned()));
    assert_eq!(d.to_string(), "mymod:mining");
    assert_eq!(d.namespaced(), ("mymod", "mining"));
    assert_eq!(Dimension::parse("custom").namespaced(), ("minecraft", "custom"));
}
