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
    if let Some(idx) = str.find(|c: char| !c.is_ascii_digit()) {
        Some((usize::from_str(&str[..idx]).ok()?, &str[idx..]))
    } else {
        // all digits!
        Some((usize::from_str(str).ok()?, ""))
    }
}

fn demangle_template_args(mut str: &str) -> Option<(&str, String)> {
    let tmpl_args = if let Some(start_idx) = str.find('<') {
        let end_idx = str.rfind('>')?;
        let mut args = &str[start_idx + 1..end_idx];
        str = &str[..start_idx];
        let mut tmpl_args = "<".to_string();
        while !args.is_empty() {
            let (arg, arg_post, rest) = demangle_arg(args)?;
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

fn demangle_name(str: &str) -> Option<(String, String, &str)> {
    let (size, rest) = parse_digits(str)?;
    // hack for template argument constants
    if rest.is_empty() || rest.starts_with(',') {
        let out = format!("{}", size);
        return Some((out.clone(), out, rest));
    }
    let (name, args) = demangle_template_args(&rest[..size])?;
    Some((name.to_string(), format!("{}{}", name, args), &rest[size..]))
}

fn demangle_qualified_name(mut str: &str) -> Option<(String, String, &str)> {
    if str.starts_with('Q') {
        let count = usize::from_str(&str[1..2]).ok()?;
        str = &str[2..];
        let mut last_class = String::new();
        let mut qualified = String::new();
        for i in 0..count {
            let (class_name, full, rest) = demangle_name(str)?;
            qualified += full.as_str();
            last_class = class_name;
            str = rest;
            if i < count - 1 {
                qualified += "::";
            }
        }
        Some((last_class, qualified, str))
    } else {
        demangle_name(str)
    }
}

fn demangle_arg(mut str: &str) -> Option<(String, String, &str)> {
    let mut result = String::new();
    let (mut pre, mut post, rest) = parse_qualifiers(str);
    result += pre.as_str();
    str = rest;
    if str.starts_with('Q') || str.starts_with(|c: char| c.is_ascii_digit()) {
        let (_, qualified, rest) = demangle_qualified_name(str)?;
        result += qualified.as_str();
        result += post.as_str();
        return Some((result, String::new(), rest));
    }
    let mut is_member = false;
    let mut const_member = false;
    if str.starts_with('M') {
        is_member = true;
        let (_, member, rest) = demangle_qualified_name(&str[1..])?;
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
        } else if post.ends_with('*') {
            post = post[..post.len() - 1].to_string();
            pre = format!("*{}", pre);
        } else {
            return None;
        }
        let (args, rest) = demangle_function_args(str)?;
        if !rest.starts_with('_') {
            return None;
        }
        let (ret_pre, ret_post, rest) = demangle_arg(&rest[1..])?;
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
        let (arg_pre, arg_post, rest) = demangle_arg(&rest[1..])?;
        if !post.is_empty() {
            post = format!("({})", post);
        }
        result = format!("{}{}{}", pre, arg_pre, post);
        let ret_post = format!("[{}]{}", count, arg_post);
        return Some((result, ret_post, rest));
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
    Some((result, String::new(), &str[1..]))
}

fn demangle_function_args(mut str: &str) -> Option<(String, &str)> {
    let mut result = String::new();
    while !str.is_empty() {
        if !result.is_empty() {
            result += ", ";
        }
        let (arg, arg_post, rest) = demangle_arg(str)?;
        result += arg.as_str();
        result += arg_post.as_str();
        str = rest;
        if str.starts_with('_') || str.starts_with(',') {
            break;
        }
    }
    Some((result, str))
}

fn demangle_special_function(str: &str, class_name: &str) -> Option<String> {
    if let Some(rest) = str.strip_prefix("op") {
        let (arg_pre, arg_post, _) = demangle_arg(rest)?;
        return Some(format!("operator {}{}", arg_pre, arg_post));
    }
    let (op, args) = demangle_template_args(str)?;
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

pub fn demangle(mut str: &str) -> Option<String> {
    let mut special = false;
    let mut cnst = false;
    let mut fn_name: String;
    let mut return_type_pre = String::new();
    let mut return_type_post = String::new();
    let mut qualified = String::new();
    if str.starts_with("__") {
        special = true;
        str = &str[2..];
    }
    {
        let idx = str.rfind("__")?;
        let (fn_name_out, rest) = str.split_at(idx);
        if special {
            fn_name = fn_name_out.to_string();
        } else {
            let (name, args) = demangle_template_args(fn_name_out)?;
            fn_name = format!("{}{}", name, args);
        }
        str = &rest[2..];
    }
    let mut class_name = String::new();
    if !str.starts_with('F') {
        let (name, qualified_name, rest) = demangle_qualified_name(str)?;
        class_name = name;
        qualified = qualified_name;
        str = rest;
    }
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
        let (ret_pre, ret_post, rest) = demangle_arg(str)?;
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
    Some(fn_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demangle_template_args() {
        assert_eq!(
            demangle_template_args("single_ptr<10CModelData>"),
            Some(("single_ptr", "<CModelData>".to_string()))
        );
        assert_eq!(
            demangle_template_args(
                "basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>"
            ),
            Some((
                "basic_string",
                "<wchar_t, rstl::char_traits<wchar_t>, rstl::rmemory_allocator>".to_string()
            ))
        );
    }

    #[test]
    fn test_demangle_name() {
        assert_eq!(
            demangle_name("24single_ptr<10CModelData>"),
            Some(("single_ptr".to_string(), "single_ptr<CModelData>".to_string(), ""))
        );
        assert_eq!(
            demangle_name("66basic_string<w,Q24rstl14char_traits<w>,Q24rstl17rmemory_allocator>"),
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
        assert_eq!(
            demangle_qualified_name("6CActor"),
            Some(("CActor".to_string(), "CActor".to_string(), ""))
        );
        assert_eq!(
            demangle_qualified_name("Q29CVector3f4EDim"),
            Some(("EDim".to_string(), "CVector3f::EDim".to_string(), ""))
        );
        assert_eq!(
            demangle_qualified_name(
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
        assert_eq!(demangle_arg("v"), Some(("void".to_string(), "".to_string(), "")));
        assert_eq!(demangle_arg("b"), Some(("bool".to_string(), "".to_string(), "")));
        assert_eq!(
            demangle_arg("RC9CVector3fUc"),
            Some(("const CVector3f&".to_string(), "".to_string(), "Uc"))
        );
        assert_eq!(
            demangle_arg("Q24rstl14char_traits<w>,"),
            Some(("rstl::char_traits<wchar_t>".to_string(), "".to_string(), ","))
        );
        assert_eq!(
            demangle_arg("PFPCcPCc_v"),
            Some(("void (*".to_string(), ")(const char*, const char*)".to_string(), ""))
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
        assert_eq!(demangle("__dt__6CActorFv"), Some("CActor::~CActor(void)".to_string()));
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
        assert_eq!(
            demangle("BareFn__FPFPCcPv_v_v"),
            Some("void BareFn(void (*)(const char*, void*))".to_string())
        );
        assert_eq!(
            demangle("BareFn__FPFPCcPv_v_PFPCvPv_v"),
            Some("void (* BareFn(void (*)(const char*, void*)))(const void*, void*)".to_string())
        );
        assert_eq!(
            demangle("SomeFn__FRCPFPFPCvPv_v_RCPFPCvPv_v"),
            Some("SomeFn(void (*const & (*const &)(void (*)(const void*, void*)))(const void*, void*))".to_string())
        );
        assert_eq!(
            demangle("SomeFn__Q29Namespace5ClassCFRCMQ29Namespace5ClassFPCvPCvMQ29Namespace5ClassFPCvPCvPCvPv_v_RCMQ29Namespace5ClassFPCvPCvPCvPv_v"),
            Some("Namespace::Class::SomeFn(void (Namespace::Class::*const & (Namespace::Class::*const &)(void (Namespace::Class::*)(const void*, void*) const) const)(const void*, void*) const) const".to_string())
        );
        assert_eq!(
            demangle("__pl__FRC9CRelAngleRC9CRelAngle"),
            Some("operator+(const CRelAngle&, const CRelAngle&)".to_string())
        );
        assert_eq!(
            demangle("destroy<PUi>__4rstlFPUiPUi"),
            Some("rstl::destroy<unsigned int*>(unsigned int*, unsigned int*)".to_string())
        );
        assert_eq!(
            demangle("__opb__33TFunctor2<CP15CGuiSliderGroup,Cf>CFv"),
            Some(
                "TFunctor2<const CGuiSliderGroup*, const float>::operator bool(void) const"
                    .to_string()
            )
        );
        assert_eq!(
            demangle("__opRC25TToken<15CCharLayoutInfo>__31TLockedToken<15CCharLayoutInfo>CFv"),
            Some("TLockedToken<CCharLayoutInfo>::operator const TToken<CCharLayoutInfo>&(void) const".to_string())
        );
        assert_eq!(
            demangle("uninitialized_copy<Q24rstl198pointer_iterator<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl89vector<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>,PQ224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo>__4rstlFQ24rstl198pointer_iterator<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl89vector<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>Q24rstl198pointer_iterator<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl89vector<Q224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo,Q24rstl17rmemory_allocator>,Q24rstl17rmemory_allocator>PQ224CSpawnSystemKeyframeData24CSpawnSystemKeyframeInfo"),
            Some("rstl::uninitialized_copy<rstl::pointer_iterator<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::vector<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::rmemory_allocator>, rstl::rmemory_allocator>, CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo*>(rstl::pointer_iterator<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::vector<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::rmemory_allocator>, rstl::rmemory_allocator>, rstl::pointer_iterator<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::vector<CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo, rstl::rmemory_allocator>, rstl::rmemory_allocator>, CSpawnSystemKeyframeData::CSpawnSystemKeyframeInfo*)".to_string())
        );
        assert_eq!(
            demangle("__rf__Q34rstl120list<Q24rstl78pair<i,PFRC10SObjectTagR12CInputStreamRC15CVParamTransfer_C16CFactoryFnReturn>,Q24rstl17rmemory_allocator>14const_iteratorCFv"),
            Some("rstl::list<rstl::pair<int, const CFactoryFnReturn (*)(const SObjectTag&, CInputStream&, const CVParamTransfer&)>, rstl::rmemory_allocator>::const_iterator::operator->(void) const".to_string())
        );
        assert_eq!(
            demangle("ApplyRipples__FRC14CRippleManagerRA43_A43_Q220CFluidPlaneCPURender13SHFieldSampleRA22_A22_UcRA256_CfRQ220CFluidPlaneCPURender10SPatchInfo"),
            Some("ApplyRipples(const CRippleManager&, CFluidPlaneCPURender::SHFieldSample(&)[43][43], unsigned char(&)[22][22], const float(&)[256], CFluidPlaneCPURender::SPatchInfo&)".to_string())
        );
        assert_eq!(
            demangle("CalculateFluidTextureOffset__14CFluidUVMotionCFfPA2_f"),
            Some(
                "CFluidUVMotion::CalculateFluidTextureOffset(float, float(*)[2]) const".to_string()
            )
        );
        assert_eq!(
            demangle("RenderNormals__FRA43_A43_CQ220CFluidPlaneCPURender13SHFieldSampleRA22_A22_CUcRCQ220CFluidPlaneCPURender10SPatchInfo"),
            Some("RenderNormals(const CFluidPlaneCPURender::SHFieldSample(&)[43][43], const unsigned char(&)[22][22], const CFluidPlaneCPURender::SPatchInfo&)".to_string())
        );
        assert_eq!(
            demangle("Matrix__FfPA2_A3_f"),
            Some("Matrix(float, float(*)[2][3])".to_string())
        );
        assert_eq!(
            demangle("__ct<12CStringTable>__31CObjOwnerDerivedFromIObjUntypedFRCQ24rstl24auto_ptr<12CStringTable>"),
            Some("CObjOwnerDerivedFromIObjUntyped::CObjOwnerDerivedFromIObjUntyped<CStringTable>(const rstl::auto_ptr<CStringTable>&)".to_string())
        );
        assert_eq!(
            demangle("__vt__40TObjOwnerDerivedFromIObj<12CStringTable>"),
            Some("TObjOwnerDerivedFromIObj<CStringTable>::__vtable".to_string())
        );
        assert_eq!(
            demangle("__RTTI__40TObjOwnerDerivedFromIObj<12CStringTable>"),
            Some("TObjOwnerDerivedFromIObj<CStringTable>::__RTTI".to_string())
        );
        assert_eq!(
            demangle("__init__mNull__Q24rstl66basic_string<c,Q24rstl14char_traits<c>,Q24rstl17rmemory_allocator>"),
            Some("rstl::basic_string<char, rstl::char_traits<char>, rstl::rmemory_allocator>::__init__mNull".to_string())
        );
    }
}
