//! Control plane API

/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use armour_lang::labels::{Label, Labels};
use armour_lang::literals::{CPID, DPID};
use armour_lang::policies::{self, OnboardingPolicy, GlobalPolicies, DPPolicies};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const CONTROL_PLANE: &str = "https://localhost:8088";
pub const TCP_PORT: u16 = 8088;


pub const ONBOARDING_POLICY_KEY : &str = "onboarding_policy";
pub const GLOBAL_POLICY_KEY : &str = "global_policy";
pub fn onboarding_policy_label() -> Label {
    Label::from_str(ONBOARDING_POLICY_KEY).unwrap()
}
pub fn global_policy_label() -> Label {
    Label::from_str(GLOBAL_POLICY_KEY).unwrap()
}


type HostCredentials = String;
type ServiceCredentials = String;
// map from domains to labels
pub type LabelMap = std::collections::BTreeMap<String, Labels>;

#[derive(Clone, Serialize, Deserialize)]
pub struct OnboardHostRequest {
    pub host: url::Url,
    pub label: Label,
    pub credentials: HostCredentials, // FIXME change types as needed
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceRequest {
    pub service: Label,
    pub host: Label,
    pub tmp_dpid: Option<DPID>,
    pub credentials: ServiceCredentials,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpecializationRequest {
    pub service: Label,
    pub proxy: Label,
    pub host: Label,
    pub policy: GlobalPolicies,
    pub cpid: CPID
}

#[derive(Serialize, Deserialize)]
pub struct SpecializationResponse {
    pub policy: DPPolicies
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceResponse {
    pub service_id: Label,
}

#[derive(Serialize, Deserialize)]
pub struct POnboardServiceRequest {
    pub service: Label,
    pub service_id: CPID, //assigned by control plane
    pub host: Label,
}

#[derive(Serialize, Deserialize)]
pub struct POnboardedServiceRequest {
    pub service: Label,
    pub host: Label,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyUpdateRequest {
    pub label: Label,
    pub policy: DPPolicies,
    pub labels: LabelMap,
}

#[derive(Serialize, Deserialize)]
pub struct CPPolicyUpdateRequest {
    pub label: Label,
    pub policy: GlobalPolicies,
    pub labels: LabelMap,
    //used to select onboarded services that need to be updated when the global policy is updated
    pub selector: Option<Label>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OnboardingUpdateRequest {
    pub label: Label,
    pub policy: OnboardingPolicy,
    pub labels: LabelMap,
}

impl OnboardingUpdateRequest {
    pub fn pack(self) -> CPPolicyUpdateRequest {
        let mut g = GlobalPolicies::new();
        g.insert(policies::Protocol::HTTP, self.policy.clone());
        CPPolicyUpdateRequest{
            label: self.label.clone(),
            policy: g,
            labels: self.labels,
            selector: None,
        }
    }
    pub fn unpack(pol: CPPolicyUpdateRequest) -> Self {
        OnboardingUpdateRequest{
            label: pol.label.clone(),
            policy: pol.policy.policy(policies::Protocol::HTTP).unwrap().clone(),
            labels: pol.labels
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryRequest {
    pub label: Label,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryResponse {
    pub policy: DPPolicies,
    pub labels: LabelMap,
}
