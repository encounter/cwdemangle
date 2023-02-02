use std::str::FromStr;

pub struct DemangleOptions {
    /// Replace `(void)` function parameters with `()`
    pub omit_empty_parameters: bool,
}

impl Default for DemangleOptions {
    fn default() -> Self { DemangleOptions { omit_empty_parameters: true } }
}

fn parse_qualifiers(mut str: &str) -> (String, String, &str) {
    let mut pre = String::new();
    let mut post = String::new();
    for c in str.chars() {
        match c {
            'P' => {
                if pre.is_empty() {
                    post.insert(0, '*');
                } else {
                    post.insert_str(0, format!("* {}", pre.trim_end()).as_str());
                    pre.clear();
                }
            }
            'R' => {
                if pre.is_empty() {
                    post.insert(0, '&');
                } else {
                    post.insert_str(0, format!("& {}", pre.trim_end()).as_str());
                    pre.clear();
                }
            }
            'C' => pre.push_str("const "),
            'V' => pre.push_str("volatile "),
            'U' => pre.push_str("unsigned "),
            'S' => pre.push_str("signed "),
            _ => break,
        };
        str = &str[1..];
    }
    post.truncate(post.trim_end().len());
    (pre, post, str)
}

fn parse_digits(str: &str) -> Option<(usize, &str)> {
    if let Some(idx) = str.find(|c: char| !c.is_ascii_digit()) {
        Some((usize::from_str(&str[..idx]).ok()?, &str[idx..]))
    } else {
        // all digits!
        Some((usize::from_str(str).ok()?, ""))
    }
}

fn demangle_template_args<'a>(
    mut str: &'a str,
    options: &DemangleOptions,
) -> Option<(&'a str, String)> {
    let tmpl_args = if let Some(start_idx) = str.find('<') {
        let end_idx = str.rfind('>')?;
        if end_idx < start_idx {
            return None;
        }
        let mut args = &str[start_idx + 1..end_idx];
        str = &str[..start_idx];
        let mut tmpl_args = "<".to_string();
        while !args.is_empty() {
            let (arg, arg_post, rest) = demangle_arg(args, options)?;
            tmpl_args += arg.as_str();
            tmpl_args += arg_post.as_str();
            if rest.is_empty() {
                break;
            } else {
                tmpl_args += ", ";
            }
            args = &rest[1..];
        }
        tmpl_args += ">";
        tmpl_args
    } else {
        String::new()
    };
    Some((str, tmpl_args))
}

fn demangle_name<'a>(str: &'a str, options: &DemangleOptions) -> Option<(String, String, &'a str)> {
    let (size, rest) = parse_digits(str)?;
    // hack for template argument constants
    if rest.is_empty() || rest.starts_with(',') {
        let out = format!("{}", size);
        return Some((out.clone(), out, rest));
    }
    if rest.len() < size {
        return None;
    }
    let (name, args) = demangle_template_args(&rest[..size], options)?;
    Some((name.to_string(), format!("{}{}", name, args), &rest[size..]))
}

fn demangle_qualified_name<'a>(
    mut str: &'a str,
    options: &DemangleOptions,
) -> Option<(String, String, &'a str)> {
    if str.starts_with('Q') {
        if str.len() < 3 {
            return None;
        }
        let count = usize::from_str(&str[1..2]).ok()?;
        str = &str[2..];
        let mut last_class = String::new();
        let mut qualified = String::new();
        for i in 0..count {
            let (class_name, full, rest) = demangle_name(str, options)?;
            qualified += full.as_str();
            last_class = class_name;
            str = rest;
            if i < count - 1 {
                qualified += "::";
            }
        }
        Some((last_class, qualified, str))
    } else {
        demangle_name(str, options)
    }
}

fn demangle_arg<'a>(
    mut str: &'a str,
    options: &DemangleOptions,
) -> Option<(String, String, &'a str)> {
    let mut result = String::new();
    let (mut pre, mut post, rest) = parse_qualifiers(str);
    result += pre.as_str();
    str = rest;
    if str.starts_with('Q') || str.starts_with(|c: char| c.is_ascii_digit()) {
        let (_, qualified, rest) = demangle_qualified_name(str, options)?;
        result += qualified.as_str();
        result += post.as_str();
        return Some((result, String::new(), rest));
    }
    let mut is_member = false;
    let mut const_member = false;
    if str.starts_with('M') {
        is_member = true;
        let (_, member, rest) = demangle_qualified_name(&str[1..], options)?;
        pre = format!("{}::*{}", member, pre);
        if !rest.starts_with('F') {
            return None;
        }
        str = rest;
    }
    if is_member || str.starts_with('F') {
        str = &str[1..];
        if is_member {
            // "const void*, const void*" or "const void*, void*"
            if str.starts_with("PCvPCv") {
                const_member = true;
                str = &str[6..];
            } else if str.starts_with("PCvPv") {
                str = &str[5..];
            } else {
                return None;
            }
        } else if post.starts_with('*') {
            post = post[1..].trim_start().to_string();
            pre = format!("*{}", pre);
        } else {
            return None;
        }
        let (args, rest) = demangle_function_args(str, options)?;
        if !rest.starts_with('_') {
            return None;
        }
        let (ret_pre, ret_post, rest) = demangle_arg(&rest[1..], options)?;
        let const_str = if const_member { " const" } else { "" };
        let res_pre = format!("{} ({}{}", ret_pre, pre, post);
        let res_post = format!(")({}){}{}", args, const_str, ret_post);
        return Some((res_pre, res_post, rest));
    }
    if let Some(rest) = str.strip_prefix('A') {
        let (count, rest) = parse_digits(rest)?;
        if !rest.starts_with('_') {
            return None;
        }
        let (arg_pre, arg_post, rest) = demangle_arg(&rest[1..], options)?;
        if !post.is_empty() {
            post = format!("({})", post);
        }
        result = format!("{}{}{}", pre, arg_pre, post);
        let ret_post = format!("[{}]{}", count, arg_post);
        return Some((result, ret_post, rest));
    }
    result.push_str(match str.chars().next()? {
        'i' => "int",
        'b' => "bool",
        'c' => "char",
        's' => "short",
        'l' => "long",
        'x' => "long long",
        'f' => "float",
        'd' => "double",
        'w' => "wchar_t",
        'v' => "void",
        'e' => "...",
        '_' => return Some((result, String::new(), rest)),
        _ => return None,
    });
    result += post.as_str();
    Some((result, String::new(), &str[1..]))
}

fn demangle_function_args<'a>(
    mut str: &'a str,
    options: &DemangleOptions,
) -> Option<(String, &'a str)> {
    let mut result = String::new();
    while !str.is_empty() {
        if !result.is_empty() {
            result += ", ";
        }
        let (arg, arg_post, rest) = demangle_arg(str, options)?;
        result += arg.as_str();
        result += arg_post.as_str();
        str = rest;
        if str.starts_with('_') || str.starts_with(',') {
            break;
        }
    }
    Some((result, str))
}

fn demangle_special_function(
    str: &str,
    class_name: &str,
    options: &DemangleOptions,
) -> Option<String> {
    if let Some(rest) = str.strip_prefix("op") {
        let (arg_pre, arg_post, _) = demangle_arg(rest, options)?;
        return Some(format!("operator {}{}", arg_pre, arg_post));
    }
    let (op, args) = demangle_template_args(str, options)?;
    Some(format!(
        "{}{}",
        match op {
            "dt" => return Some(format!("~{}{}", class_name, args)),
            "ct" => class_name,
            "nw" => "operator new",
            "nwa" => "operator new[]",
            "dl" => "operator delete",
            "dla" => "operator delete[]",
            "pl" => "operator+",
            "mi" => "operator-",
            "ml" => "operator*",
            "dv" => "operator/",
            "md" => "operator%",
            "er" => "operator^",
            "ad" => "operator&",
            "or" => "operator|",
            "co" => "operator~",
            "nt" => "operator!",
            "as" => "operator=",
            "lt" => "operator<",
            "gt" => "operator>",
            "apl" => "operator+=",
            "ami" => "operator-=",
            "amu" => "operator*=",
            "adv" => "operator/=",
            "amd" => "operator%=",
            "aer" => "operator^=",
            "aad" => "operator&=",
            "aor" => "operator|=",
            "ls" => "operator<<",
            "rs" => "operator>>",
            "ars" => "operator>>=",
            "als" => "operator<<=",
            "eq" => "operator==",
            "ne" => "operator!=",
            "le" => "operator<=",
            "ge" => "operator>=",
            "aa" => "operator&&",
            "oo" => "operator||",
            "pp" => "operator++",
            "mm" => "operator--",
            "cm" => "operator,",
            "rm" => "operator->*",
            "rf" => "operator->",
            "cl" => "operator()",
            "vc" => "operator[]",
            "vt" => "__vtable",
            _ => return Some(format!("__{}{}", op, args)),
        },
        args
    ))
}

pub fn demangle(mut str: &str, options: &DemangleOptions) -> Option<String> {
    if !str.is_ascii() {
        return None;
    }

    let mut special = false;
    let mut cnst = false;
    let mut fn_name: String;
    let mut return_type_pre = String::new();
    let mut return_type_post = String::new();
    let mut qualified = String::new();
    let mut static_var = String::new();

    // Handle new static function variables (Wii CW)
    let guard = str.starts_with("@GUARD@");
    if guard || str.starts_with("@LOCAL@") {
        str = &str[7..];
        let idx = str.rfind('@')?;
        let (rest, var) = str.split_at(idx);
        if guard {
            static_var = format!("{} guard", &var[1..]);
        } else {
            static_var = var[1..].to_string();
        }
        str = rest;
    }

    if str.starts_with("__") {
        special = true;
        str = &str[2..];
    }
    {
        let idx = str.find("__")?;
        let (fn_name_out, mut rest) = str.split_at(idx);
        if special {
            if fn_name_out == "init" {
                // Special case for double __
                let rest_idx = rest[2..].find("__")?;
                fn_name = str[..rest_idx + 6].to_string();
                rest = &rest[rest_idx + 2..];
            } else {
                fn_name = fn_name_out.to_string();
            }
        } else {
            let (name, args) = demangle_template_args(fn_name_out, options)?;
            fn_name = format!("{}{}", name, args);
        }

        // Handle old static function variables (GC CW)
        if let Some(first_idx) = fn_name.find('$') {
            let second_idx = fn_name[first_idx + 1..].find('$')?;
            let (var, rest) = fn_name.split_at(first_idx);
            let (var_type, rest) = rest[1..].split_at(second_idx);
            if !var_type.starts_with("localstatic") {
                return None;
            }
            if var == "init" {
                // Sadly, $localstatic doesn't provide the variable name in guard/init
                static_var = format!("{} guard", var_type);
            } else {
                static_var = var.to_string();
            }
            fn_name = rest[1..].to_string();
        }

        str = &rest[2..];
    }
    let mut class_name = String::new();
    if !str.starts_with('F') {
        let (name, qualified_name, rest) = demangle_qualified_name(str, options)?;
        class_name = name;
        qualified = qualified_name;
        str = rest;
    }
    if special {
        fn_name = demangle_special_function(fn_name.as_str(), class_name.as_str(), options)?;
    }
    if str.starts_with('C') {
        str = &str[1..];
        cnst = true;
    }
    if str.starts_with('F') {
        str = &str[1..];
        let (args, rest) = demangle_function_args(str, options)?;
        if options.omit_empty_parameters && args == "void" {
            fn_name = format!("{}()", fn_name);
        } else {
            fn_name = format!("{}({})", fn_name, args);
        }
        str = rest;
    }
    if str.starts_with('_') {
        str = &str[1..];
        let (ret_pre, ret_post, rest) = demangle_arg(str, options)?;
        return_type_pre = ret_pre;
        return_type_post = ret_post;
        str = rest;
    }
    if !str.is_empty() {
        return None;
    }
    if cnst {
        fn_name = format!("{} const", fn_name);
    }
    if !qualified.is_empty() {
        fn_name = format!("{}::{}", qualified, fn_name);
    }
    if !return_type_pre.is_empty() {
        fn_name = format!("{} {}{}", return_type_pre, fn_name, return_type_post);
    }
    if !static_var.is_empty() {
        fn_name = format!("{}::{}", fn_name, static_var);
    }
    Some(fn_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demangle_template_args() {
        let options = DemangleOptions::default();
        assert_eq!(
            demangle_template_args("single_ptr<10CModelData>", &options),
            Some(("single_ptr", "<CModelData>".to_string()))
        );
        assert_eq!(
            demangle_template_args(
                "basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>",
                &options
            ),
            Some((
                "basic_string",
                "<wchar_t, rstl::char_traits<wchar_t>, rstl::rmemory_allocator>".to_string()
            ))
        );
    }

    #[test]
    fn test_demangle_name() {
        let options = DemangleOptions::default();
        assert_eq!(
            demangle_name("24single_ptr<10CModelData>", &options),
            Some(("single_ptr".to_string(), "single_ptr<CModelData>".to_string(), ""))
        );
        assert_eq!(
            demangle_name(
                "66basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>",
                &options
            ),
            Some((
                "basic_string".to_string(),
                "basic_string<wchar_t, rstl::char_traits<wchar_t>, rstl::rmemory_allocator>"
                    .to_string(),
                ""
            ))
        );
    }

    #[test]
    fn test_demangle_qualified_name() {
        let options = DemangleOptions::default();
        assert_eq!(
            demangle_qualified_name("6CActor", &options),
            Some(("CActor".to_string(), "CActor".to_string(), ""))
        );
        assert_eq!(
            demangle_qualified_name("Q29CVector3f4EDim", &options),
            Some(("EDim".to_string(), "CVector3f::EDim".to_string(), ""))
        );
        assert_eq!(
            demangle_qualified_name(
                "Q24rstl66basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>",
                &options
            ),
            Some((
                "basic_string".to_string(),
                "rstl::basic_string<wchar_t, rstl::char_traits<wchar_t>, rstl::rmemory_allocator>"
                    .to_string(),
                ""
            ))
        )
    }

    #[test]
    fn test_demangle_arg() {
        let options = DemangleOptions::default();
        assert_eq!(demangle_arg("v", &options), Some(("void".to_string(), "".to_string(), "")));
        assert_eq!(demangle_arg("b", &options), Some(("bool".to_string(), "".to_string(), "")));
        assert_eq!(
            demangle_arg("RC9CVector3fUc", &options),
            Some(("const CVector3f&".to_string(), "".to_string(), "Uc"))
        );
        assert_eq!(
            demangle_arg("Q24rstl14char_traits<w>,", &options),
            Some(("rstl::char_traits<wchar_t>".to_string(), "".to_string(), ","))
        );
        assert_eq!(
            demangle_arg("PFPCcPCc_v", &options),
            Some(("void (*".to_string(), ")(const char*, const char*)".to_string(), ""))
        );
        assert_eq!(
            demangle_arg("RCPCVPCVUi", &options),
            Some((
                "const volatile unsigned int* const volatile* const&".to_string(),
                "".to_string(),
                ""
            ))
        );
    }

    #[test]
    fn test_demangle_function_args() {
        let options = DemangleOptions::default();
        assert_eq!(demangle_function_args("v", &options), Some(("void".to_string(), "")));
        assert_eq!(demangle_function_args("b", &options), Some(("bool".to_string(), "")));
        assert_eq!(
            demangle_function_args("RC9CVector3fUc_x", &options),
            Some(("const CVector3f&, unsigned char".to_string(), "_x"))
        );
    }

    #[test]
    fn test_demangle() {
        let options = DemangleOptions::default();
        assert_eq!(demangle("cfunction", &options), None);
        assert_eq!(demangle("__dt__6CActorFv", &options), Some("CActor::~CActor()".to_string()));
        assert_eq!(
            demangle("GetSfxHandle__6CActorCFv", &options),
            Some("CActor::GetSfxHandle() const".to_string())
        );
        assert_eq!(
            demangle("mNull__Q24rstl66basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>", &options),
            Some("rstl::basic_string<wchar_t, rstl::char_traits<wchar_t>, rstl::rmemory_allocator>::mNull".to_string())
        );
        assert_eq!(
            demangle("__ct__Q34rstl495red_black_tree<Ux,Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>,0,Q24rstl215select1st<Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>>,Q24rstl8less<Ux>,Q24rstl17rmemory_allocator>8iteratorFPQ34rstl495red_black_tree<Ux,Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>,0,Q24rstl215select1st<Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>>,Q24rstl8less<Ux>,Q24rstl17rmemory_allocator>4nodePCQ34rstl495red_black_tree<Ux,Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>,0,Q24rstl215select1st<Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>>,Q24rstl8less<Ux>,Q24rstl17rmemory_allocator>6header", &options),
            Some("rstl::red_black_tree<unsigned long long, rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>, 0, rstl::select1st<rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>>, rstl::less<unsigned long long>, rstl::rmemory_allocator>::iterator::iterator(rstl::red_black_tree<unsigned long long, rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>, 0, rstl::select1st<rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>>, rstl::less<unsigned long long>, rstl::rmemory_allocator>::node*, const rstl::red_black_tree<unsigned long long, rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>, 0, rstl::select1st<rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>>, rstl::less<unsigned long long>, rstl::rmemory_allocator>::header*)".to_string()),
        );
        assert_eq!(
            demangle("for_each<PP12MultiEmitter,Q23std51binder2nd<Q23std30mem_fun1_t<v,12MultiEmitter,l>,l>>__3stdFPP12MultiEmitterPP12MultiEmitterQ23std51binder2nd<Q23std30mem_fun1_t<v,12MultiEmitter,l>,l>_Q23std51binder2nd<Q23std30mem_fun1_t<v,12MultiEmitter,l>,l>", &options),
            Some("std::binder2nd<std::mem_fun1_t<void, MultiEmitter, long>, long> std::for_each<MultiEmitter**, std::binder2nd<std::mem_fun1_t<void, MultiEmitter, long>, long>>(MultiEmitter**, MultiEmitter**, std::binder2nd<std::mem_fun1_t<void, MultiEmitter, long>, long>)".to_string())
        );
        assert_eq!(
            demangle("__ct__Q43std3tr16detail383function_imp<PFPCcPCc_v,Q43std3tr16detail334bound_func<v,Q43std3tr16detail59mem_fn_2<v,Q53scn4step7gimmick9shipevent9ShipEvent,PCc,PCc>,Q33std3tr1228tuple<PQ53scn4step7gimmick9shipevent9ShipEvent,Q53std3tr112placeholders6detail5ph<1>,Q53std3tr112placeholders6detail5ph<2>,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat>>,0,1>FRCQ43std3tr16detail383function_imp<PFPCcPCc_v,Q43std3tr16detail334bound_func<v,Q43std3tr16detail59mem_fn_2<v,Q53scn4step7gimmick9shipevent9ShipEvent,PCc,PCc>,Q33std3tr1228tuple<PQ53scn4step7gimmick9shipevent9ShipEvent,Q53std3tr112placeholders6detail5ph<1>,Q53std3tr112placeholders6detail5ph<2>,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat>>,0,1>", &options),
            Some("std::tr1::detail::function_imp<void (*)(const char*, const char*), std::tr1::detail::bound_func<void, std::tr1::detail::mem_fn_2<void, scn::step::gimmick::shipevent::ShipEvent, const char*, const char*>, std::tr1::tuple<scn::step::gimmick::shipevent::ShipEvent*, std::tr1::placeholders::detail::ph<1>, std::tr1::placeholders::detail::ph<2>, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat>>, 0, 1>::function_imp(const std::tr1::detail::function_imp<void (*)(const char*, const char*), std::tr1::detail::bound_func<void, std::tr1::detail::mem_fn_2<void, scn::step::gimmick::shipevent::ShipEvent, const char*, const char*>, std::tr1::tuple<scn::step::gimmick::shipevent::ShipEvent*, std::tr1::placeholders::detail::ph<1>, std::tr1::placeholders::detail::ph<2>, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat>>, 0, 1>&)".to_string())
        );
        assert_eq!(
            demangle("createJointController<11IKJointCtrl>__2MRFP11IKJointCtrlPC9LiveActorUsM11IKJointCtrlFPCvPvPQ29JGeometry64TPosition3<Q29JGeometry38TMatrix34<Q29JGeometry13SMatrix34C<f>>>RC19JointControllerInfo_bM11IKJointCtrlFPCvPvPQ29JGeometry64TPosition3<Q29JGeometry38TMatrix34<Q29JGeometry13SMatrix34C<f>>>RC19JointControllerInfo_b_P15JointController", &options),
            Some("JointController* MR::createJointController<IKJointCtrl>(IKJointCtrl*, const LiveActor*, unsigned short, bool (IKJointCtrl::*)(JGeometry::TPosition3<JGeometry::TMatrix34<JGeometry::SMatrix34C<float>>>*, const JointControllerInfo&), bool (IKJointCtrl::*)(JGeometry::TPosition3<JGeometry::TMatrix34<JGeometry::SMatrix34C<float>>>*, const JointControllerInfo&))".to_string())
        );
        assert_eq!(
            demangle("execCommand__12JASSeqParserFP8JASTrackM12JASSeqParserFPCvPvP8JASTrackPUl_lUlPUl", &options),
            Some("JASSeqParser::execCommand(JASTrack*, long (JASSeqParser::*)(JASTrack*, unsigned long*), unsigned long, unsigned long*)".to_string())
        );
        assert_eq!(
            demangle("AddWidgetFnMap__10CGuiWidgetFiM10CGuiWidgetFPCvPvP15CGuiFunctionDefP18CGuiControllerInfo_i", &options),
            Some("CGuiWidget::AddWidgetFnMap(int, int (CGuiWidget::*)(CGuiFunctionDef*, CGuiControllerInfo*))".to_string())
        );
        assert_eq!(
            demangle("BareFn__FPFPCcPv_v_v", &options),
            Some("void BareFn(void (*)(const char*, void*))".to_string())
        );
        assert_eq!(
            demangle("BareFn__FPFPCcPv_v_PFPCvPv_v", &options),
            Some("void (* BareFn(void (*)(const char*, void*)))(const void*, void*)".to_string())
        );
        assert_eq!(
            demangle("SomeFn__FRCPFPFPCvPv_v_RCPFPCvPv_v", &options),
            Some("SomeFn(void (*const& (*const&)(void (*)(const void*, void*)))(const void*, void*))".to_string())
        );
        assert_eq!(
            demangle("SomeFn__Q29Namespace5ClassCFRCMQ29Namespace5ClassFPCvPCvMQ29Namespace5ClassFPCvPCvPCvPv_v_RCMQ29Namespace5ClassFPCvPCvPCvPv_v", &options),
            Some("Namespace::Class::SomeFn(void (Namespace::Class::*const & (Namespace::Class::*const &)(void (Namespace::Class::*)(const void*, void*) const) const)(const void*, void*) const) const".to_string())
        );
        assert_eq!(
            demangle("__pl__FRC9CRelAngleRC9CRelAngle", &options),
            Some("operator+(const CRelAngle&, const CRelAngle&)".to_string())
        );
        assert_eq!(
            demangle("destroy<PUi>__4rstlFPUiPUi", &options),
            Some("rstl::destroy<unsigned int*>(unsigned int*, unsigned int*)".to_string())
        );
        assert_eq!(
            demangle("__opb__33TFunctor2<CP15CGuiSliderGroup,Cf>CFv", &options),
            Some(
                "TFunctor2<CGuiSliderGroup* const, const float>::operator bool() const".to_string()
            )
        );
        assert_eq!(
            demangle(
                "__opRC25TToken<15CCharLayoutInfo>__31TLockedToken<15CCharLayoutInfo>CFv",
                &options
            ),
            Some(
                "TLockedToken<CCharLayoutInfo>::operator const TToken<CCharLayoutInfo>&() const"
                    .to_string()
            )
        );
        assert_eq!(
            demangle("uninitialized_copy<Q24rstl198pointer_iterator<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl89vector<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>,PQ224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo>__4rstlFQ24rstl198pointer_iterator<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl89vector<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>Q24rstl198pointer_iterator<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl89vector<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>PQ224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo", &options),
            Some("rstl::uninitialized_copy<rstl::pointer_iterator<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::vector<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::rmemory_allocator>, rstl::rmemory_allocator>, CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo*>(rstl::pointer_iterator<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::vector<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::rmemory_allocator>, rstl::rmemory_allocator>, rstl::pointer_iterator<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::vector<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::rmemory_allocator>, rstl::rmemory_allocator>, CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo*)".to_string())
        );
        assert_eq!(
            demangle("__rf__Q34rstl120list<Q24rstl78pair<i,PFRC10SObjectTagR12CInputStreamRC15CVParamTransfer_C16CFactoryFnReturn>,Q24rstl17rmemory_allocator>14const_iteratorCFv", &options),
            Some("rstl::list<rstl::pair<int, const CFactoryFnReturn (*)(const SObjectTag&, CInputStream&, const CVParamTransfer&)>, rstl::rmemory_allocator>::const_iterator::operator->() const".to_string())
        );
        assert_eq!(
            demangle("ApplyRipples__FRC14CRippleManagerRA43_A43_Q220CFluidPlaneCPURender13SHFieldSampleRA22_A22_UcRA256_CfRQ220CFluidPlaneCPURender10SPatchInfo", &options),
            Some("ApplyRipples(const CRippleManager&, CFluidPlaneCPURender::SHFieldSample(&)[43][43], unsigned char(&)[22][22], const float(&)[256], CFluidPlaneCPURender::SPatchInfo&)".to_string())
        );
        assert_eq!(
            demangle("CalculateFluidTextureOffset__14CFluidUVMotionCFfPA2_f", &options),
            Some(
                "CFluidUVMotion::CalculateFluidTextureOffset(float, float(*)[2]) const".to_string()
            )
        );
        assert_eq!(
            demangle("RenderNormals__FRA43_A43_CQ220CFluidPlaneCPURender13SHFieldSampleRA22_A22_CUcRCQ220CFluidPlaneCPURender10SPatchInfo", &options),
            Some("RenderNormals(const CFluidPlaneCPURender::SHFieldSample(&)[43][43], const unsigned char(&)[22][22], const CFluidPlaneCPURender::SPatchInfo&)".to_string())
        );
        assert_eq!(
            demangle("Matrix__FfPA2_A3_f", &options),
            Some("Matrix(float, float(*)[2][3])".to_string())
        );
        assert_eq!(
            demangle("__ct<12CStringTable>__31CObjOwnerDerivedFromIObjUntypedFRCQ24rstl24auto_ptr<12CStringTable>", &options),
            Some("CObjOwnerDerivedFromIObjUntyped::CObjOwnerDerivedFromIObjUntyped<CStringTable>(const rstl::auto_ptr<CStringTable>&)".to_string())
        );
        assert_eq!(
            demangle("__vt__40TObjOwnerDerivedFromIObj<12CStringTable>", &options),
            Some("TObjOwnerDerivedFromIObj<CStringTable>::__vtable".to_string())
        );
        assert_eq!(
            demangle("__RTTI__40TObjOwnerDerivedFromIObj<12CStringTable>", &options),
            Some("TObjOwnerDerivedFromIObj<CStringTable>::__RTTI".to_string())
        );
        assert_eq!(
            demangle("__init__mNull__Q24rstl66basic_string<c,Q24rstl14char_traits<c>,Q24rstl17rmemory_allocator>", &options),
            Some("rstl::basic_string<char, rstl::char_traits<char>, rstl::rmemory_allocator>::__init__mNull".to_string())
        );
        assert_eq!(
            demangle("__dt__26__partial_array_destructorFv", &options),
            Some("__partial_array_destructor::~__partial_array_destructor()".to_string())
        );
        assert_eq!(
            demangle("__distance<Q34rstl195red_black_tree<13TGameScriptId,Q24rstl32pair<13TGameScriptId,9TUniqueId>,1,Q24rstl52select1st<Q24rstl32pair<13TGameScriptId,9TUniqueId>>,Q24rstl21less<13TGameScriptId>,Q24rstl17rmemory_allocator>14const_iterator>__4rstlFQ34rstl195red_black_tree<13TGameScriptId,Q24rstl32pair<13TGameScriptId,9TUniqueId>,1,Q24rstl52select1st<Q24rstl32pair<13TGameScriptId,9TUniqueId>>,Q24rstl21less<13TGameScriptId>,Q24rstl17rmemory_allocator>14const_iteratorQ34rstl195red_black_tree<13TGameScriptId,Q24rstl32pair<13TGameScriptId,9TUniqueId>,1,Q24rstl52select1st<Q24rstl32pair<13TGameScriptId,9TUniqueId>>,Q24rstl21less<13TGameScriptId>,Q24rstl17rmemory_allocator>14const_iteratorQ24rstl20forward_iterator_tag", &options),
            Some("rstl::__distance<rstl::red_black_tree<TGameScriptId, rstl::pair<TGameScriptId, TUniqueId>, 1, rstl::select1st<rstl::pair<TGameScriptId, TUniqueId>>, rstl::less<TGameScriptId>, rstl::rmemory_allocator>::const_iterator>(rstl::red_black_tree<TGameScriptId, rstl::pair<TGameScriptId, TUniqueId>, 1, rstl::select1st<rstl::pair<TGameScriptId, TUniqueId>>, rstl::less<TGameScriptId>, rstl::rmemory_allocator>::const_iterator, rstl::red_black_tree<TGameScriptId, rstl::pair<TGameScriptId, TUniqueId>, 1, rstl::select1st<rstl::pair<TGameScriptId, TUniqueId>>, rstl::less<TGameScriptId>, rstl::rmemory_allocator>::const_iterator, rstl::forward_iterator_tag)".to_string())
        );
        assert_eq!(
            demangle("__ct__Q210Metrowerks683compressed_pair<RQ23std301allocator<Q33std276__tree_deleter<Q23std34pair<Ci,Q212petfurniture8Instance>,Q33std131__multimap_do_transform<i,Q212petfurniture8Instance,Q23std7less<i>,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>,0>13value_compare,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>>4node>,Q210Metrowerks337compressed_pair<Q210Metrowerks12number<Ul,1>,PQ33std276__tree_deleter<Q23std34pair<Ci,Q212petfurniture8Instance>,Q33std131__multimap_do_transform<i,Q212petfurniture8Instance,Q23std7less<i>,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>,0>13value_compare,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>>4node>>FRQ23std301allocator<Q33std276__tree_deleter<Q23std34pair<Ci,Q212petfurniture8Instance>,Q33std131__multimap_do_transform<i,Q212petfurniture8Instance,Q23std7less<i>,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>,0>13value_compare,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>>4node>Q210Metrowerks337compressed_pair<Q210Metrowerks12number<Ul,1>,PQ33std276__tree_deleter<Q23std34pair<Ci,Q212petfurniture8Instance>,Q33std131__multimap_do_transform<i,Q212petfurniture8Instance,Q23std7less<i>,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>,0>13value_compare,Q23std53allocator<Q23std34pair<Ci,Q212petfurniture8Instance>>>4node>", &options),
            Some("Metrowerks::compressed_pair<std::allocator<std::__tree_deleter<std::pair<const int, petfurniture::Instance>, std::__multimap_do_transform<int, petfurniture::Instance, std::less<int>, std::allocator<std::pair<const int, petfurniture::Instance>>, 0>::value_compare, std::allocator<std::pair<const int, petfurniture::Instance>>>::node>&, Metrowerks::compressed_pair<Metrowerks::number<unsigned long, 1>, std::__tree_deleter<std::pair<const int, petfurniture::Instance>, std::__multimap_do_transform<int, petfurniture::Instance, std::less<int>, std::allocator<std::pair<const int, petfurniture::Instance>>, 0>::value_compare, std::allocator<std::pair<const int, petfurniture::Instance>>>::node*>>::compressed_pair(std::allocator<std::__tree_deleter<std::pair<const int, petfurniture::Instance>, std::__multimap_do_transform<int, petfurniture::Instance, std::less<int>, std::allocator<std::pair<const int, petfurniture::Instance>>, 0>::value_compare, std::allocator<std::pair<const int, petfurniture::Instance>>>::node>&, Metrowerks::compressed_pair<Metrowerks::number<unsigned long, 1>, std::__tree_deleter<std::pair<const int, petfurniture::Instance>, std::__multimap_do_transform<int, petfurniture::Instance, std::less<int>, std::allocator<std::pair<const int, petfurniture::Instance>>, 0>::value_compare, std::allocator<std::pair<const int, petfurniture::Instance>>>::node*>)".to_string())
        );
        assert_eq!(
            demangle("skBadString$localstatic3$GetNameByToken__31TTokenSet<18EScriptObjectState>CF18EScriptObjectState", &options),
            Some("TTokenSet<EScriptObjectState>::GetNameByToken(EScriptObjectState) const::skBadString".to_string())
        );
        assert_eq!(
            demangle("init$localstatic4$GetNameByToken__31TTokenSet<18EScriptObjectState>CF18EScriptObjectState", &options),
            Some("TTokenSet<EScriptObjectState>::GetNameByToken(EScriptObjectState) const::localstatic4 guard".to_string())
        );
        assert_eq!(
            demangle(
                "@LOCAL@GetAnmPlayPolicy__Q24nw4r3g3dFQ34nw4r3g3d9AnmPolicy@policyTable",
                &options
            ),
            Some("nw4r::g3d::GetAnmPlayPolicy(nw4r::g3d::AnmPolicy)::policyTable".to_string())
        );
        assert_eq!(
            demangle(
                "@GUARD@GetAnmPlayPolicy__Q24nw4r3g3dFQ34nw4r3g3d9AnmPolicy@policyTable",
                &options
            ),
            Some(
                "nw4r::g3d::GetAnmPlayPolicy(nw4r::g3d::AnmPolicy)::policyTable guard".to_string()
            )
        );
        // Truncated symbol
        assert_eq!(
            demangle("lower_bound<Q24rstl180const_pointer_iterator<Q24rstl33pair<Ui,22CAdditiveAnimationInfo>,Q24rstl77vector<Q24rstl33pair<Ui,22CAdditiveAnimationInfo>,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>,Ui,Q24rstl79pair_sorter_finder<Q24rstl33pair<Ui,22CAdditiveAnimationInfo>,Q24rstl8less<Ui>>>__4rstlFQ24rstl180const_pointer_iterator<Q24rstl33pair<Ui,22CAdditiveAnimationInfo>,Q24rstl77vector<Q24rstl33pair<Ui,22CAdditiveAnimationInfo>,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>Q24rstl180const_p", &options),
            None
        );
        assert_eq!(
            demangle("test__FRCPCPCi", &options),
            Some("test(const int* const* const&)".to_string()),
        );
        assert_eq!(
            demangle(
                "__ct__Q34nw4r2ut14CharStrmReaderFMQ34nw4r2ut14CharStrmReaderFPCvPv_Us",
                &options
            ),
            Some("nw4r::ut::CharStrmReader::CharStrmReader(unsigned short (nw4r::ut::CharStrmReader::*)())".to_string())
        );
    }

    #[test]
    fn test_demangle_options() {
        let options = DemangleOptions { omit_empty_parameters: true };
        assert_eq!(
            demangle("__dt__26__partial_array_destructorFv", &options),
            Some("__partial_array_destructor::~__partial_array_destructor()".to_string())
        );
        let options = DemangleOptions { omit_empty_parameters: false };
        assert_eq!(
            demangle("__dt__26__partial_array_destructorFv", &options),
            Some("__partial_array_destructor::~__partial_array_destructor(void)".to_string())
        );
    }
}
