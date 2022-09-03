use std::str::FromStr;

fn parse_qualifiers(mut str: &str) -> (String, String, &str) {
    let mut pre = String::new();
    let mut post = String::new();
    for c in str.chars() {
        match c {
            'P' => post.push('*'),
            'R' => post.push('&'),
            'C' => pre.push_str("const "),
            'U' => pre.push_str("unsigned "),
            'S' => pre.push_str("signed "),
            _ => break,
        };
        str = &str[1..];
    }
    (pre, post, str)
}

fn parse_digits(str: &str) -> Option<(usize, &str)> {
    if let Some(idx) = str.find(|c: char| !c.is_digit(10)) {
        Some((usize::from_str(&str[..idx]).ok()?, &str[idx..]))
    } else {
        // all digits!
        Some((usize::from_str(str).ok()?, ""))
    }
}

fn demange_template_args(mut str: &str) -> Option<(&str, String)> {
    let qualified = if let Some(start_idx) = str.find('<') {
        let end_idx = str.rfind('>')?;
        let mut args = &str[start_idx + 1..end_idx];
        str = &str[..start_idx];
        let mut qualified = str.to_string();
        qualified += "<";
        while !args.is_empty() {
            let (arg, rest) = demangle_arg(args)?;
            qualified += arg.as_str();
            if rest.is_empty() {
                break;
            } else {
                qualified += ", ";
            }
            args = &rest[1..];
        }
        qualified += ">";
        qualified
    } else {
        str.to_string()
    };
    Some((str, qualified))
}

fn demangle_class(str: &str) -> Option<(String, String, &str)> {
    let (size, rest) = parse_digits(str)?;
    // hack for template argument constants
    if rest.is_empty() || rest.starts_with(',') {
        let out = format!("{}", size);
        return Some((out.clone(), out, rest));
    }
    let (class_name, qualified) = demange_template_args(&rest[..size])?;
    Some((class_name.to_string(), qualified, &rest[size..]))
}

fn demangle_qualified_class(mut str: &str) -> Option<(String, String, &str)> {
    if str.starts_with('Q') {
        let count = usize::from_str(&str[1..2]).ok()?;
        str = &str[2..];
        let mut last_class = String::new();
        let mut qualified = String::new();
        for i in 0..count {
            let (class_name, full, rest) = demangle_class(str)?;
            qualified += full.as_str();
            last_class = class_name;
            str = rest;
            if i < count - 1 {
                qualified += "::";
            }
        }
        Some((last_class, qualified, str))
    } else {
        demangle_class(str)
    }
}

fn demangle_arg(mut str: &str) -> Option<(String, &str)> {
    let mut result = String::new();
    let (pre, mut post, rest) = parse_qualifiers(str);
    result += pre.as_str();
    str = rest;
    if str.starts_with('Q') || str.starts_with(|c: char| c.is_digit(10)) {
        let (_, qualified, rest) = demangle_qualified_class(str)?;
        result += qualified.as_str();
        result += post.as_str();
        return Some((result, rest));
    }
    let mut is_member = false;
    if str.starts_with('M') {
        is_member = true;
        let (_, member, rest) = demangle_qualified_class(&str[1..])?;
        post = format!("{}::*{}", member, post);
        if !rest.starts_with('F') {
            return None;
        }
        str = rest;
    }
    if is_member || str.starts_with('F') {
        str = &str[1..];
        if is_member {
            // Member functions always(?) include "const void*, void*"
            if !str.starts_with("PCvPv") {
                return None;
            }
            str = &str[5..];
        }
        let (args, rest) = demangle_function_args(str)?;
        if !rest.starts_with('_') {
            return None;
        }
        let (ret, rest) = demangle_arg(&rest[1..])?;
        result += format!("{} ({})({})", ret, post, args).as_str();
        return Some((result, rest));
    }
    if str.starts_with('A') {
        todo!("array")
    }
    result.push_str(match str.chars().next().unwrap() {
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
        _ => return None,
    });
    result += post.as_str();
    Some((result, &str[1..]))
}

fn demangle_function_args(mut str: &str) -> Option<(String, &str)> {
    let mut result = String::new();
    while !str.is_empty() {
        if !result.is_empty() {
            result += ", ";
        }
        let (arg, rest) = demangle_arg(str)?;
        result += arg.as_str();
        str = rest;
        if str.starts_with('_') || str.starts_with(',') {
            break;
        }
    }
    Some((result, str))
}

fn demangle_special_function(str: &str, class_name: &str) -> Option<String> {
    Some(
        match str {
            "dt" => return Some("~".to_string() + class_name),
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
            _ => return None,
        }
            .to_string(),
    )
}

pub fn demangle(mut str: &str) -> Option<String> {
    let mut special = false;
    let mut cnst = false;
    let mut fn_name: String;
    if str.starts_with("__") {
        special = true;
        str = &str[2..];
    }
    {
        let idx = str.find("__")?;
        let (fn_name_out, rest) = str.split_at(idx);
        let (_, qualified) = demange_template_args(fn_name_out)?;
        fn_name = qualified;
        str = &rest[2..];
    }
    let (class_name, mut qualified, rest) = demangle_qualified_class(str)?;
    str = rest;
    if special {
        fn_name = demangle_special_function(fn_name.as_str(), class_name.as_str())?;
    }
    if str.starts_with('C') {
        str = &str[1..];
        cnst = true;
    }
    if str.starts_with('F') {
        str = &str[1..];
        let (args, rest) = demangle_function_args(str)?;
        fn_name = format!("{}({})", fn_name, args);
        str = rest;
    }
    if str.starts_with('_') {
        str = &str[1..];
        let (ret, rest) = demangle_arg(str)?;
        qualified = format!("{} {}", ret, qualified);
        str = rest;
    }
    if !str.is_empty() {
        return None;
    }
    if cnst {
        fn_name = format!("{} const", fn_name);
    }
    if !qualified.is_empty() {
        return Some(format!("{}::{}", qualified, fn_name));
    }
    Some(fn_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demangle_class() {
        assert_eq!(
            demangle_class("24single_ptr<10CModelData>"),
            Some((
                "single_ptr".to_string(),
                "single_ptr<CModelData>".to_string(),
                ""
            ))
        )
    }

    #[test]
    fn test_demangle_qualified_class() {
        assert_eq!(
            demangle_qualified_class("6CActor"),
            Some(("CActor".to_string(), "CActor".to_string(), ""))
        );
        assert_eq!(
            demangle_qualified_class("Q29CVector3f4EDim"),
            Some(("EDim".to_string(), "CVector3f::EDim".to_string(), ""))
        );
        assert_eq!(
            demangle_qualified_class(
                "Q24rstl66basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>"
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
        assert_eq!(demangle_arg("v"), Some(("void".to_string(), "")));
        assert_eq!(demangle_arg("b"), Some(("bool".to_string(), "")));
        assert_eq!(
            demangle_arg("RC9CVector3fUc"),
            Some(("const CVector3f&".to_string(), "Uc"))
        );
        assert_eq!(
            demangle_arg("Q24rstl14char_traits<w>,"),
            Some(("rstl::char_traits<wchar_t>".to_string(), ","))
        );
        assert_eq!(
            demangle_arg("PFPCcPCc_v"),
            Some(("void (*)(const char*, const char*)".to_string(), ""))
        )
    }

    #[test]
    fn test_demangle_function_args() {
        assert_eq!(demangle_function_args("v"), Some(("void".to_string(), "")));
        assert_eq!(demangle_function_args("b"), Some(("bool".to_string(), "")));
        assert_eq!(
            demangle_function_args("RC9CVector3fUc_x"),
            Some(("const CVector3f&, unsigned char".to_string(), "_x"))
        );
    }

    #[test]
    fn test_demangle() {
        assert_eq!(demangle("cfunction"), None);
        assert_eq!(
            demangle("__dt__6CActorFv"),
            Some("CActor::~CActor(void)".to_string())
        );
        assert_eq!(
            demangle("GetSfxHandle__6CActorCFv"),
            Some("CActor::GetSfxHandle(void) const".to_string())
        );
        assert_eq!(
            demangle("mNull__Q24rstl66basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>"),
            Some("rstl::basic_string<wchar_t, rstl::char_traits<wchar_t>, rstl::rmemory_allocator>::mNull".to_string())
        );
        assert_eq!(
            demangle("__ct__Q34rstl495red_black_tree<Ux,Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>,0,Q24rstl215select1st<Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>>,Q24rstl8less<Ux>,Q24rstl17rmemory_allocator>8iteratorFPQ34rstl495red_black_tree<Ux,Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>,0,Q24rstl215select1st<Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>>,Q24rstl8less<Ux>,Q24rstl17rmemory_allocator>4nodePCQ34rstl495red_black_tree<Ux,Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>,0,Q24rstl215select1st<Q24rstl194pair<Ux,Q24rstl175auto_ptr<Q24rstl155map<s,Q24rstl96auto_ptr<Q24rstl77list<Q24rstl35auto_ptr<23CGuiFrameMessageMapNode>,Q24rstl17rmemory_allocator>>,Q24rstl7less<s>,Q24rstl17rmemory_allocator>>>>,Q24rstl8less<Ux>,Q24rstl17rmemory_allocator>6header"),
            Some("rstl::red_black_tree<unsigned long long, rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>, 0, rstl::select1st<rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>>, rstl::less<unsigned long long>, rstl::rmemory_allocator>::iterator::iterator(rstl::red_black_tree<unsigned long long, rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>, 0, rstl::select1st<rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>>, rstl::less<unsigned long long>, rstl::rmemory_allocator>::node*, const rstl::red_black_tree<unsigned long long, rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>, 0, rstl::select1st<rstl::pair<unsigned long long, rstl::auto_ptr<rstl::map<short, rstl::auto_ptr<rstl::list<rstl::auto_ptr<CGuiFrameMessageMapNode>, rstl::rmemory_allocator>>, rstl::less<short>, rstl::rmemory_allocator>>>>, rstl::less<unsigned long long>, rstl::rmemory_allocator>::header*)".to_string()),
        );
        assert_eq!(
            demangle("for_each<PP12MultiEmitter,Q23std51binder2nd<Q23std30mem_fun1_t<v,12MultiEmitter,l>,l>>__3stdFPP12MultiEmitterPP12MultiEmitterQ23std51binder2nd<Q23std30mem_fun1_t<v,12MultiEmitter,l>,l>_Q23std51binder2nd<Q23std30mem_fun1_t<v,12MultiEmitter,l>,l>"),
            Some("std::binder2nd<std::mem_fun1_t<void, MultiEmitter, long>, long> std::for_each<MultiEmitter**, std::binder2nd<std::mem_fun1_t<void, MultiEmitter, long>, long>>(MultiEmitter**, MultiEmitter**, std::binder2nd<std::mem_fun1_t<void, MultiEmitter, long>, long>)".to_string())
        );
        assert_eq!(
            demangle("__ct__Q43std3tr16detail383function_imp<PFPCcPCc_v,Q43std3tr16detail334bound_func<v,Q43std3tr16detail59mem_fn_2<v,Q53scn4step7gimmick9shipevent9ShipEvent,PCc,PCc>,Q33std3tr1228tuple<PQ53scn4step7gimmick9shipevent9ShipEvent,Q53std3tr112placeholders6detail5ph<1>,Q53std3tr112placeholders6detail5ph<2>,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat>>,0,1>FRCQ43std3tr16detail383function_imp<PFPCcPCc_v,Q43std3tr16detail334bound_func<v,Q43std3tr16detail59mem_fn_2<v,Q53scn4step7gimmick9shipevent9ShipEvent,PCc,PCc>,Q33std3tr1228tuple<PQ53scn4step7gimmick9shipevent9ShipEvent,Q53std3tr112placeholders6detail5ph<1>,Q53std3tr112placeholders6detail5ph<2>,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat,Q33std3tr13nat>>,0,1>"),
            Some("std::tr1::detail::function_imp<void (*)(const char*, const char*), std::tr1::detail::bound_func<void, std::tr1::detail::mem_fn_2<void, scn::step::gimmick::shipevent::ShipEvent, const char*, const char*>, std::tr1::tuple<scn::step::gimmick::shipevent::ShipEvent*, std::tr1::placeholders::detail::ph<1>, std::tr1::placeholders::detail::ph<2>, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat>>, 0, 1>::function_imp(const std::tr1::detail::function_imp<void (*)(const char*, const char*), std::tr1::detail::bound_func<void, std::tr1::detail::mem_fn_2<void, scn::step::gimmick::shipevent::ShipEvent, const char*, const char*>, std::tr1::tuple<scn::step::gimmick::shipevent::ShipEvent*, std::tr1::placeholders::detail::ph<1>, std::tr1::placeholders::detail::ph<2>, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat, std::tr1::nat>>, 0, 1>&)".to_string())
        );
        assert_eq!(
            demangle("createJointController<11IKJointCtrl>__2MRFP11IKJointCtrlPC9LiveActorUsM11IKJointCtrlFPCvPvPQ29JGeometry64TPosition3<Q29JGeometry38TMatrix34<Q29JGeometry13SMatrix34C<f>>>RC19JointControllerInfo_bM11IKJointCtrlFPCvPvPQ29JGeometry64TPosition3<Q29JGeometry38TMatrix34<Q29JGeometry13SMatrix34C<f>>>RC19JointControllerInfo_b_P15JointController"),
            Some("JointController* MR::createJointController<IKJointCtrl>(IKJointCtrl*, const LiveActor*, unsigned short, bool (IKJointCtrl::*)(JGeometry::TPosition3<JGeometry::TMatrix34<JGeometry::SMatrix34C<float>>>*, const JointControllerInfo&), bool (IKJointCtrl::*)(JGeometry::TPosition3<JGeometry::TMatrix34<JGeometry::SMatrix34C<float>>>*, const JointControllerInfo&))".to_string())
        );
        assert_eq!(
            demangle("execCommand__12JASSeqParserFP8JASTrackM12JASSeqParserFPCvPvP8JASTrackPUl_lUlPUl"),
            Some("JASSeqParser::execCommand(JASTrack*, long (JASSeqParser::*)(JASTrack*, unsigned long*), unsigned long, unsigned long*)".to_string())
        );
        assert_eq!(
            demangle("AddWidgetFnMap__10CGuiWidgetFiM10CGuiWidgetFPCvPvP15CGuiFunctionDefP18CGuiControllerInfo_i"),
            Some("CGuiWidget::AddWidgetFnMap(int, int (CGuiWidget::*)(CGuiFunctionDef*, CGuiControllerInfo*))".to_string())
        );
    }
}
