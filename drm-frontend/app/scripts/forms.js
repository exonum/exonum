// (function() {
//   Backbone.Form.template = templates.form;
//   Backbone.Form.Field.template = templates.formField;
//   Backbone.Form.editors.Base.prototype.className = 'form-control';
//   Backbone.Form.Field.errorClassName = 'has-error';

//   Backbone.Form.validators.confirmPassword = function(fieldName) {
//     return function(value, formValues) {
//       if (value != formValues[fieldName]) {
//         return {
//           type: 'password',
//           message: 'Password does not match'
//         }
//       }
//     }
//   }
// })();


// var LoginForm = Backbone.Form.extend({

//   events: {
//     "submit": "submit"
//   },

//   schema: {
//       username: {
//         type: 'Text',
//         title: "Username",
//         validators: ['required']
//       },
//       password: {
//         type: 'Password',
//         title: "Password",
//         validators: ['required']
//       }
//   },

//   templateData: {
//     legend: "Login",
//     submitButton: "Login"
//   },

//   submit: function(e) {
//     e.preventDefault()

//     var errors = this.validate();
//     if (errors) {
//       return errors;
//     }

//     app.login(this.getValue());
//   }

// });

// var RegistrationForm = Backbone.Form.extend({

//   events: {
//     "submit": "submit"
//   },

//   schema: {
//       email: {
//         type: 'Text',
//         title: "Email",
//         dataType: 'email',
//         validators: ['required', 'email']
//       },
//       username: {
//         type: 'Text',
//         title: "Username",
//         validators: ['required']
//       },
//       password: {
//         type: 'Password',
//         title: "Password",
//         validators: ['required']
//       },
//       confirm: {
//         type: 'Password',
//         title: "Confirm Password",
//         validators: ['required', Backbone.Form.validators.confirmPassword("password")]
//       },
//       // app_url: {
//       //   type: 'Text',
//       //   title: "Your application",
//       //   help: "Link to application on Google Play",
//       //   dataType: 'url',
//       //   validators: ['url']
//       // }
//   },

//   templateData: {
//     legend: "Registration",
//     submitButton: "Start Now"
//   },

//   submit: function(e) {
//     e.preventDefault()

//     var errors = this.validate();
//     if (errors) {
//       return errors;
//     }

//     app.registration(this.getValue());
//   }

// });

// var UpdateProfileForm = Backbone.Form.extend({

//   events: {
//     "submit": "submit"
//   },

//   schema: {
//       full_name: {
//         type: 'Text',
//         title: "Full Name"
//       },
//       email: {
//         type: 'Text',
//         title: "Email",
//         dataType: 'email',
//         validators: ['required', 'email']
//       },
//       country: {
//         type: 'Text',
//         title: "Country"
//       },
//       company: {
//         type: 'Text',
//         title: "Company"
//       }
//   },

//   templateData: {
//     legend: undefined,
//     submitButton: "Update profile"
//   },

//   submit: function(e) {
//     e.preventDefault()

//     var errors = this.validate();
//     if (errors) {
//       return errors;
//     }

//     app.updateProfile(this.getValue());
//   }

// });


// var ChangePasswordForm = Backbone.Form.extend({

//   events: {
//     "submit": "submit"
//   },

//   schema: {
//       old_password: {
//         type: 'Password',
//         title: "Old Password",
//         validators: ['required']
//       },
//       new_password: {
//         type: 'Password',
//         title: "New Password",
//         validators: ['required']
//       },
//       confirm: {
//         type: 'Password',
//         title: "Confirm Password",
//         validators: ['required', Backbone.Form.validators.confirmPassword("new_password")]
//       }
//   },

//   templateData: {
//     legend: undefined,
//     submitButton: "Change Password"
//   },

//   submit: function(e) {
//     e.preventDefault()

//     var errors = this.validate();
//     if (errors) {
//       return errors;
//     }

//     app.changePassword(this.getValue());
//   }

// });


// var AddApplicationForm = Backbone.Form.extend({

//   template: templates.modalForm,

//   events: {
//     "submit": "submit"
//   },

//   schema: {
//       android_package: {
//         type: 'Text',
//         validators: ['required'],
//         template: templates.modalFormField,
//         editorAttrs: {
//           placeholder: "Android Package Name"
//         }
//       }
//   },

//   templateData: {
//     legend: "Add Application",
//     submitButton: "Add Application",
//   },

//   submit: function(e) {
//     e.preventDefault()

//     var errors = this.validate();
//     if (errors) {
//       return errors;
//     }

//     this.$el.modal('hide');

//     app.addApplication(this.getValue());
//   }

// });
